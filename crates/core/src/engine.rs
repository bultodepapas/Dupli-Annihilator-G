use crate::{
    cancel::{ensure_not_canceled, CancelCheck, NoCancel},
    config::{Config, DiskAlphabeticalMode, Mode, OutputOrdering},
    dedupe_ram::RamStore,
    disk::WritableBuckets,
    disk_sort,
    progress::{ProgressEvent, ProgressSink},
    rich_input_resolver::{resolve_rich_inputs, RichInputResolution},
    stats::{AutoDecisionTelemetry, FileStats, Stats},
    text_line_reader::LossyLineReader,
    token_iter::TokenIter,
    writer::OutputWriter,
};
use ahash::RandomState;
use hashbrown::HashSet;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::time::Instant;

const IO_BUFFER_BYTES: usize = 64 * 1024;
const SAMPLE_PER_FILE_BYTES: u64 = 256 * 1024;
const SAMPLE_TOTAL_BYTES: u64 = 1024 * 1024;
const SAMPLE_MAX_FILES: usize = 4;
const MIN_STORE_RESERVE: usize = 16 * 1024;
const MAX_STORE_RESERVE: usize = 25_000_000;
const AUTO_HOST_SAFETY_MIN_BYTES: u64 = 1024 * 1024 * 1024;
const AUTO_HOST_SAFETY_RATIO: f64 = 0.20;
const AUTO_DISK_RAM_PRESSURE_RATIO: f64 = 0.70;
const AUTO_SINGLE_FILE_RAM_RATIO: f64 = 0.50;
const AUTO_LOW_USABLE_MEMORY_BYTES: u64 = 512 * 1024 * 1024;
const AUTO_LOW_MEMORY_INPUT_BYTES: u64 = 64 * 1024 * 1024;
const AUTO_PARTIAL_OVERLAP_INPUT_BYTES: u64 = 128 * 1024 * 1024;
const AUTO_DUPLICATE_RATIO_MIN: f64 = 0.10;
const AUTO_DUPLICATE_RATIO_MAX: f64 = 0.90;
const AUTO_ENTRY_OVERHEAD_BYTES: f64 = 56.0;
const AUTO_RAM_HEADROOM_RATIO: f64 = 1.20;

#[derive(Debug, Clone, Copy)]
struct MemorySnapshot {
    available_bytes: u64,
    free_bytes: u64,
    total_bytes: u64,
}

trait MemoryProbe {
    fn snapshot(&self) -> MemorySnapshot;
}

#[derive(Debug, Clone, Copy)]
struct SystemMemoryProbe;

impl MemoryProbe for SystemMemoryProbe {
    fn snapshot(&self) -> MemorySnapshot {
        use sysinfo::{MemoryRefreshKind, RefreshKind, System};

        let sys = System::new_with_specifics(
            RefreshKind::nothing().with_memory(MemoryRefreshKind::everything()),
        );

        MemorySnapshot {
            available_bytes: sys.available_memory(),
            free_bytes: sys.free_memory(),
            total_bytes: sys.total_memory(),
        }
    }
}

#[derive(Debug, Clone)]
struct WorkloadSample {
    total_input_bytes: u64,
    sampled_bytes: u64,
    sample_tokens: u64,
    sample_unique_tokens: u64,
    sample_duplicate_tokens: u64,
    avg_token_bytes: f64,
    sample_unique_ratio: f64,
    sample_duplicate_ratio: f64,
    sample_token_density: f64,
    reserve_hint: usize,
}

impl WorkloadSample {
    fn empty(total_input_bytes: u64) -> Self {
        Self {
            total_input_bytes,
            sampled_bytes: 0,
            sample_tokens: 0,
            sample_unique_tokens: 0,
            sample_duplicate_tokens: 0,
            avg_token_bytes: 0.0,
            sample_unique_ratio: 1.0,
            sample_duplicate_ratio: 0.0,
            sample_token_density: 0.0,
            reserve_hint: MIN_STORE_RESERVE,
        }
    }
}

#[derive(Debug, Clone)]
struct ExecutionPlan {
    chosen_mode: Mode,
    reserve_hint: usize,
    auto_telemetry: Option<AutoDecisionTelemetry>,
}

pub fn run<P: ProgressSink>(config: &Config, progress: P) -> anyhow::Result<Stats> {
    run_with_control(config, progress, NoCancel)
}

pub fn run_with_control<P: ProgressSink, C: CancelCheck>(
    config: &Config,
    progress: P,
    cancel: C,
) -> anyhow::Result<Stats> {
    config.validate()?;
    ensure_not_canceled(&cancel)?;

    let RichInputResolution {
        resolved_config,
        temp_files: _temp_files,
        failures,
        path_aliases,
    } = resolve_rich_inputs(config, &progress, &cancel)?;

    let plan = plan_execution(&resolved_config, &SystemMemoryProbe);
    progress.on_event(ProgressEvent::ModeResolved(plan.chosen_mode));
    let mut stats = match plan.chosen_mode {
        Mode::Ram => run_ram(
            &resolved_config,
            &progress,
            &cancel,
            &path_aliases,
            plan.reserve_hint,
        ),
        Mode::Disk => run_disk(&resolved_config, &progress, &cancel),
        Mode::Auto => unreachable!("effective_mode never returns Mode::Auto"),
    }?;

    stats.mode_effective = Some(plan.chosen_mode);
    stats.auto_telemetry = plan.auto_telemetry;
    stats.failed_pdfs = failures;
    Ok(stats)
}

/// Returns the mode that will actually be used when running `config`.
///
/// For [`Mode::Ram`] and [`Mode::Disk`] this is the mode itself. For
/// [`Mode::Auto`] the function samples the workload after any rich-input
/// extraction, inspects current host memory, and estimates the RAM cost of
/// the dedupe set before deciding between RAM and DISK.
pub fn effective_mode(config: &Config) -> Mode {
    plan_execution(config, &SystemMemoryProbe).chosen_mode
}

fn plan_execution(config: &Config, probe: &impl MemoryProbe) -> ExecutionPlan {
    let sample = sample_workload(config);
    let reserve_hint = sample.reserve_hint.max(MIN_STORE_RESERVE);

    match config.mode {
        Mode::Ram => ExecutionPlan {
            chosen_mode: Mode::Ram,
            reserve_hint,
            auto_telemetry: None,
        },
        Mode::Disk => ExecutionPlan {
            chosen_mode: Mode::Disk,
            reserve_hint,
            auto_telemetry: None,
        },
        Mode::Auto => {
            let (chosen_mode, telemetry) = decide_auto_mode(config, &sample, probe.snapshot());
            ExecutionPlan {
                chosen_mode,
                reserve_hint,
                auto_telemetry: Some(telemetry),
            }
        }
    }
}

fn sample_workload(config: &Config) -> WorkloadSample {
    let total_input_bytes = total_input_bytes(config);
    let mut sampled = WorkloadSample::empty(total_input_bytes);
    let mut sampled_token_bytes: u64 = 0;
    let mut seen: HashSet<Box<str>, RandomState> = HashSet::with_hasher(RandomState::new());
    let mut remaining_total = SAMPLE_TOTAL_BYTES;

    for path in config.inputs.iter().take(SAMPLE_MAX_FILES) {
        if remaining_total == 0 {
            break;
        }

        let file = match File::open(path) {
            Ok(file) => file,
            Err(_) => continue,
        };

        let mut reader = LossyLineReader::new(BufReader::with_capacity(IO_BUFFER_BYTES, file));
        let mut line = String::new();
        let mut sampled_from_file = 0u64;
        let per_file_limit = SAMPLE_PER_FILE_BYTES.min(remaining_total);

        loop {
            if sampled_from_file >= per_file_limit || remaining_total == 0 {
                break;
            }

            let n = match reader.read_line(&mut line) {
                Ok(n) => n,
                Err(_) => break,
            };
            if n == 0 {
                break;
            }

            let line_bytes = n as u64;
            sampled_from_file = sampled_from_file.saturating_add(line_bytes);
            sampled.sampled_bytes = sampled.sampled_bytes.saturating_add(line_bytes);
            remaining_total = remaining_total.saturating_sub(line_bytes);

            for raw in TokenIter::new(&line) {
                let mut token = raw;
                if config.trim {
                    token = token.trim();
                }
                if config.drop_empty && token.is_empty() {
                    continue;
                }
                if config.should_drop_by_length(token) {
                    continue;
                }

                sampled.sample_tokens += 1;
                sampled_token_bytes += token.len() as u64;
                if seen.insert(token.into()) {
                    sampled.sample_unique_tokens += 1;
                } else {
                    sampled.sample_duplicate_tokens += 1;
                }
            }
        }
    }

    if sampled.sample_tokens > 0 {
        sampled.avg_token_bytes = sampled_token_bytes as f64 / sampled.sample_tokens as f64;
        sampled.sample_unique_ratio =
            sampled.sample_unique_tokens as f64 / sampled.sample_tokens as f64;
        sampled.sample_duplicate_ratio =
            sampled.sample_duplicate_tokens as f64 / sampled.sample_tokens as f64;
    }
    if sampled.sampled_bytes > 0 {
        sampled.sample_token_density = sampled.sample_tokens as f64 / sampled.sampled_bytes as f64;
    }

    let estimated_total_tokens = estimate_total_tokens(&sampled);
    let estimated_unique_tokens =
        (estimated_total_tokens * sampled.sample_unique_ratio).ceil() as usize;
    sampled.reserve_hint = estimated_unique_tokens.clamp(MIN_STORE_RESERVE, MAX_STORE_RESERVE);
    sampled
}

fn total_input_bytes(config: &Config) -> u64 {
    config
        .inputs
        .iter()
        .filter_map(|p| std::fs::metadata(p).ok())
        .map(|m| m.len())
        .sum()
}

fn estimate_total_tokens(sample: &WorkloadSample) -> f64 {
    if sample.sample_token_density > 0.0 {
        sample.total_input_bytes as f64 * sample.sample_token_density
    } else if sample.avg_token_bytes > 0.0 {
        sample.total_input_bytes as f64 / sample.avg_token_bytes.max(4.0)
    } else {
        0.0
    }
}

fn estimate_ram_bytes(sample: &WorkloadSample) -> u64 {
    let estimated_total_tokens = estimate_total_tokens(sample);
    let estimated_unique_tokens = estimated_total_tokens * sample.sample_unique_ratio;
    let avg_token_bytes = sample.avg_token_bytes.max(4.0);
    let raw = estimated_unique_tokens * (avg_token_bytes + AUTO_ENTRY_OVERHEAD_BYTES);
    (raw * AUTO_RAM_HEADROOM_RATIO).ceil() as u64
}

fn decide_auto_mode(
    config: &Config,
    sample: &WorkloadSample,
    memory: MemorySnapshot,
) -> (Mode, AutoDecisionTelemetry) {
    let effective_available_bytes = if memory.available_bytes > 0 {
        memory.available_bytes
    } else {
        memory.free_bytes
    };
    let safety_margin_bytes =
        AUTO_HOST_SAFETY_MIN_BYTES.max((memory.total_bytes as f64 * AUTO_HOST_SAFETY_RATIO) as u64);
    let usable_memory_bytes = effective_available_bytes.saturating_sub(safety_margin_bytes);
    let estimated_ram_bytes = estimate_ram_bytes(sample);
    let pressure_threshold =
        (usable_memory_bytes as f64 * AUTO_DISK_RAM_PRESSURE_RATIO).round() as u64;
    let single_file_ram_threshold =
        (usable_memory_bytes as f64 * AUTO_SINGLE_FILE_RAM_RATIO).round() as u64;

    let (chosen_mode, decision_reason) = if memory.available_bytes == 0 {
        if effective_available_bytes > 0
            && sample.total_input_bytes > effective_available_bytes.saturating_div(2)
        {
            (Mode::Disk, "available_memory_zero_fallback_to_free_memory")
        } else if memory.total_bytes > 0 && sample.total_input_bytes > memory.total_bytes / 2 {
            (Mode::Disk, "available_memory_zero_fallback_to_total_memory")
        } else {
            (Mode::Ram, "available_memory_zero_fallback_ram")
        }
    } else if pressure_threshold > 0 && estimated_ram_bytes > pressure_threshold {
        (Mode::Disk, "estimated_ram_exceeds_70pct_usable_memory")
    } else if usable_memory_bytes < AUTO_LOW_USABLE_MEMORY_BYTES
        && sample.total_input_bytes > AUTO_LOW_MEMORY_INPUT_BYTES
    {
        (Mode::Disk, "usable_memory_below_512mb_with_large_input")
    } else if config.inputs.len() >= 2
        && sample.total_input_bytes > AUTO_PARTIAL_OVERLAP_INPUT_BYTES
        && (AUTO_DUPLICATE_RATIO_MIN..=AUTO_DUPLICATE_RATIO_MAX)
            .contains(&sample.sample_duplicate_ratio)
        && matches!(config.ordering, OutputOrdering::PreserveFirstSeen)
    {
        (Mode::Disk, "multi_file_partial_overlap_prefers_disk")
    } else if config.inputs.len() == 1
        && single_file_ram_threshold > 0
        && estimated_ram_bytes <= single_file_ram_threshold
    {
        (Mode::Ram, "single_file_within_50pct_usable_memory")
    } else if usable_memory_bytes > 0 && estimated_ram_bytes <= usable_memory_bytes {
        (Mode::Ram, "estimated_ram_fits_usable_memory")
    } else if usable_memory_bytes == 0 && sample.total_input_bytes <= AUTO_LOW_MEMORY_INPUT_BYTES {
        (Mode::Ram, "usable_memory_zero_but_input_small")
    } else {
        (Mode::Disk, "conservative_disk_fallback")
    };

    (
        chosen_mode,
        AutoDecisionTelemetry {
            available_memory_bytes: memory.available_bytes,
            total_memory_bytes: memory.total_bytes,
            usable_memory_bytes,
            safety_margin_bytes,
            estimated_ram_bytes,
            sample_tokens: sample.sample_tokens,
            sample_unique_ratio: sample.sample_unique_ratio,
            sample_duplicate_ratio: sample.sample_duplicate_ratio,
            decision_reason: decision_reason.to_string(),
        },
    )
}

fn run_ram<P: ProgressSink, C: CancelCheck>(
    config: &Config,
    progress: &P,
    cancel: &C,
    path_aliases: &HashMap<PathBuf, PathBuf>,
    reserve_hint: usize,
) -> anyhow::Result<Stats> {
    let started = Instant::now();
    progress.on_event(ProgressEvent::Stage("Tokenizing"));

    let mut store = match config.ordering {
        OutputOrdering::UnorderedFast => RamStore::new_unordered(),
        OutputOrdering::PreserveFirstSeen | OutputOrdering::Alphabetical => RamStore::new_stable(),
    };

    store.reserve(reserve_hint.max(MIN_STORE_RESERVE));

    let mut stats = Stats {
        files: config.inputs.len(),
        per_file: if config.per_file_stats {
            Some(Vec::with_capacity(config.inputs.len()))
        } else {
            None
        },
        ..Default::default()
    };

    for (idx, path) in config.inputs.iter().enumerate() {
        ensure_not_canceled(cancel)?;
        progress.on_event(ProgressEvent::FileStarted {
            index: idx + 1,
            total: config.inputs.len(),
        });

        let mut pf_tokens_seen: u64 = 0;
        let mut pf_duplicates: u64 = 0;
        let mut pf_unique_new: u64 = 0;
        let mut pf_filtered: u64 = 0;

        let file = File::open(path)?;
        let mut reader = LossyLineReader::new(BufReader::with_capacity(IO_BUFFER_BYTES, file));
        let mut line = String::new();

        loop {
            ensure_not_canceled(cancel)?;
            let n = reader.read_line(&mut line)?;
            if n == 0 {
                break;
            }

            for raw in TokenIter::new(&line) {
                stats.tokens_seen += 1;
                pf_tokens_seen += 1;
                if stats.tokens_seen % 8_192 == 0 {
                    ensure_not_canceled(cancel)?;
                }
                if stats.tokens_seen % 100_000 == 0 {
                    progress.on_event(ProgressEvent::TokensSeen(stats.tokens_seen));
                }

                let mut token = raw;
                if config.trim {
                    token = token.trim();
                }
                if config.drop_empty && token.is_empty() {
                    continue;
                }
                if config.should_drop_by_length(token) {
                    stats.filtered_by_length += 1;
                    pf_filtered += 1;
                    if stats.filtered_by_length % 100_000 == 0 {
                        progress
                            .on_event(ProgressEvent::FilteredByLength(stats.filtered_by_length));
                    }
                    continue;
                }

                if store.insert(token) {
                    stats.unique_tokens += 1;
                    pf_unique_new += 1;
                    if stats.unique_tokens % 100_000 == 0 {
                        progress.on_event(ProgressEvent::UniqueTokens(stats.unique_tokens));
                    }
                } else {
                    stats.duplicates += 1;
                    pf_duplicates += 1;
                    if stats.duplicates % 100_000 == 0 {
                        progress.on_event(ProgressEvent::Duplicates(stats.duplicates));
                    }
                }
            }
        }

        if let Some(ref mut per_file) = stats.per_file {
            let display_path = path_aliases
                .get(path)
                .cloned()
                .unwrap_or_else(|| path.clone());
            let file_bytes = std::fs::metadata(&display_path).map(|m| m.len()).ok();
            per_file.push(FileStats {
                path: display_path,
                file_bytes,
                tokens_seen: pf_tokens_seen,
                duplicates: pf_duplicates,
                unique_new: pf_unique_new,
                filtered_by_length: pf_filtered,
            });
        }

        progress.on_event(ProgressEvent::FileFinished {
            index: idx + 1,
            total: config.inputs.len(),
        });
    }

    ensure_not_canceled(cancel)?;
    let mut tokens = store.into_tokens();
    if matches!(config.ordering, OutputOrdering::Alphabetical) {
        progress.on_event(ProgressEvent::Stage("Sorting"));
        tokens.sort_unstable();
    }

    ensure_not_canceled(cancel)?;
    progress.on_event(ProgressEvent::Stage("WritingOutput"));
    let mut out = OutputWriter::create(&config.output, config.output_separator.clone())?;
    for token in tokens {
        ensure_not_canceled(cancel)?;
        out.write_token(&token)?;
    }
    out.finish()?;

    progress.on_event(ProgressEvent::Stage("Finalizing"));
    stats.elapsed = started.elapsed();
    Ok(stats)
}

fn run_disk<P: ProgressSink, C: CancelCheck>(
    config: &Config,
    progress: &P,
    cancel: &C,
) -> anyhow::Result<Stats> {
    let started = Instant::now();
    let mut stats = Stats {
        files: config.inputs.len(),
        ..Default::default()
    };

    if matches!(config.ordering, OutputOrdering::Alphabetical) {
        match config.disk_alphabetical_mode {
            DiskAlphabeticalMode::FastBucketLocal => {
                let buckets = WritableBuckets::new(config.disk_buckets)?;
                let buckets = buckets.partition_inputs(config, progress, &mut stats, cancel)?;
                buckets.reduce_to_output(config, progress, &mut stats, cancel)?;
            }
            DiskAlphabeticalMode::GlobalPerfect => {
                let temp = tempfile::tempdir()?;
                disk_sort::external_sort_global(config, progress, &mut stats, temp.path(), cancel)?;
            }
        }
    } else {
        let buckets = WritableBuckets::new(config.disk_buckets)?;
        let buckets = buckets.partition_inputs(config, progress, &mut stats, cancel)?;
        buckets.reduce_to_output(config, progress, &mut stats, cancel)?;
    }

    progress.on_event(ProgressEvent::Stage("Finalizing"));
    stats.elapsed = started.elapsed();
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::{
        decide_auto_mode, sample_workload, MemorySnapshot, WorkloadSample,
        AUTO_LOW_MEMORY_INPUT_BYTES,
    };
    use crate::config::{Config, DiskAlphabeticalMode, Mode, OutputOrdering};
    use std::fs;

    fn make_cfg(inputs: Vec<std::path::PathBuf>) -> Config {
        Config {
            inputs,
            output: std::env::temp_dir().join("dedupe-auto-test-out.txt"),
            output_separator: "\n".to_string(),
            mode: Mode::Auto,
            ordering: OutputOrdering::PreserveFirstSeen,
            trim: true,
            drop_empty: true,
            drop_length_min: None,
            drop_length_max: None,
            disk_buckets: 64,
            disk_alphabetical_mode: DiskAlphabeticalMode::FastBucketLocal,
            disk_run_bytes: 2 * 1024 * 1024,
            per_file_stats: false,
        }
    }

    #[test]
    fn auto_prefers_ram_for_single_unique_workload_with_room() {
        let sample = WorkloadSample {
            total_input_bytes: 32 * 1024 * 1024,
            sampled_bytes: 256 * 1024,
            sample_tokens: 20_000,
            sample_unique_tokens: 19_500,
            sample_duplicate_tokens: 500,
            avg_token_bytes: 9.0,
            sample_unique_ratio: 0.975,
            sample_duplicate_ratio: 0.025,
            sample_token_density: 0.076,
            reserve_hint: 100_000,
        };
        let cfg = make_cfg(vec![std::env::temp_dir().join("single.txt")]);
        let (mode, telemetry) = decide_auto_mode(
            &cfg,
            &sample,
            MemorySnapshot {
                available_bytes: 8 * 1024 * 1024 * 1024,
                free_bytes: 8 * 1024 * 1024 * 1024,
                total_bytes: 16 * 1024 * 1024 * 1024,
            },
        );

        assert_eq!(mode, Mode::Ram);
        assert_eq!(
            telemetry.decision_reason,
            "single_file_within_50pct_usable_memory"
        );
    }

    #[test]
    fn auto_prefers_disk_for_multi_file_partial_overlap() {
        let sample = WorkloadSample {
            total_input_bytes: 256 * 1024 * 1024,
            sampled_bytes: 512 * 1024,
            sample_tokens: 50_000,
            sample_unique_tokens: 30_000,
            sample_duplicate_tokens: 20_000,
            avg_token_bytes: 8.0,
            sample_unique_ratio: 0.60,
            sample_duplicate_ratio: 0.40,
            sample_token_density: 0.095,
            reserve_hint: 500_000,
        };
        let cfg = make_cfg(vec![
            std::env::temp_dir().join("a.txt"),
            std::env::temp_dir().join("b.txt"),
        ]);
        let (mode, telemetry) = decide_auto_mode(
            &cfg,
            &sample,
            MemorySnapshot {
                available_bytes: 4 * 1024 * 1024 * 1024,
                free_bytes: 4 * 1024 * 1024 * 1024,
                total_bytes: 16 * 1024 * 1024 * 1024,
            },
        );

        assert_eq!(mode, Mode::Disk);
        assert!(
            telemetry.decision_reason == "estimated_ram_exceeds_70pct_usable_memory"
                || telemetry.decision_reason == "multi_file_partial_overlap_prefers_disk"
        );
    }

    #[test]
    fn auto_prefers_disk_when_usable_memory_is_low() {
        let sample = WorkloadSample {
            total_input_bytes: AUTO_LOW_MEMORY_INPUT_BYTES + 1,
            sampled_bytes: 64 * 1024,
            sample_tokens: 8_000,
            sample_unique_tokens: 7_500,
            sample_duplicate_tokens: 500,
            avg_token_bytes: 7.0,
            sample_unique_ratio: 0.9375,
            sample_duplicate_ratio: 0.0625,
            sample_token_density: 0.12,
            reserve_hint: 50_000,
        };
        let cfg = make_cfg(vec![std::env::temp_dir().join("single.txt")]);
        let (mode, telemetry) = decide_auto_mode(
            &cfg,
            &sample,
            MemorySnapshot {
                available_bytes: 1400 * 1024 * 1024,
                free_bytes: 1400 * 1024 * 1024,
                total_bytes: 4 * 1024 * 1024 * 1024,
            },
        );

        assert_eq!(mode, Mode::Disk);
        assert!(
            telemetry.decision_reason == "estimated_ram_exceeds_70pct_usable_memory"
                || telemetry.decision_reason == "usable_memory_below_512mb_with_large_input"
        );
    }

    #[test]
    fn sampling_applies_current_filters() {
        let dir = tempfile::tempdir().expect("tempdir");
        let input = dir.path().join("sample.txt");
        fs::write(&input, " a,bb ; \nccc;d;d\n").expect("write sample");

        let mut cfg = make_cfg(vec![input]);
        cfg.drop_length_min = Some(1);
        cfg.drop_length_max = Some(1);

        let sample = sample_workload(&cfg);
        assert_eq!(sample.sample_tokens, 2);
        assert_eq!(sample.sample_unique_tokens, 2);
        assert_eq!(sample.sample_duplicate_tokens, 0);
    }
}
