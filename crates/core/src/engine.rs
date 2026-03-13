use crate::{
    cancel::{ensure_not_canceled, CancelCheck, NoCancel},
    config::{Config, DiskAlphabeticalMode, Mode, OutputOrdering},
    dedupe_ram::RamStore,
    disk::WritableBuckets,
    disk_sort,
    epub_reader,
    pdf_reader,
    progress::{ProgressEvent, ProgressSink},
    stats::{FileStats, Stats},
    text_line_reader::LossyLineReader,
    token_iter::TokenIter,
    writer::OutputWriter,
};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::time::Instant;
use tempfile::NamedTempFile;

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

    // Resolve any PDF/EPUB inputs to temporary plain-text files before the engine
    // loop runs.  The returned `_temp_files` vec keeps the NamedTempFiles alive
    // for the entire duration of the job; they are deleted when it drops.
    let (resolved, _temp_files, failed_pdfs, path_aliases) =
        resolve_rich_inputs(config, &progress, &cancel)?;

    let chosen = effective_mode(&resolved);
    progress.on_event(ProgressEvent::ModeResolved(chosen));
    let mut stats = match chosen {
        Mode::Ram => run_ram(&resolved, &progress, &cancel, &path_aliases),
        Mode::Disk => run_disk(&resolved, &progress, &cancel),
        Mode::Auto => unreachable!("effective_mode never returns Mode::Auto"),
    }?;

    stats.mode_effective = Some(chosen);
    stats.failed_pdfs = failed_pdfs;
    Ok(stats)
}

/// Returns the mode that will actually be used when running `config`.
///
/// For [`Mode::Ram`] and [`Mode::Disk`] this is the mode itself.  For
/// [`Mode::Auto`] the function queries available system memory and compares it
/// against the total size of the input files:
///
/// - If inputs exceed **50 %** of available RAM the engine switches to
///   [`Mode::Disk`] to avoid OOM conditions (the hash-set overhead typically
///   adds 1.5–2× the raw token volume on top of the input size).
/// - Otherwise [`Mode::Ram`] is chosen.
///
/// The function never returns [`Mode::Auto`].
pub fn effective_mode(config: &Config) -> Mode {
    match config.mode {
        Mode::Ram | Mode::Disk => config.mode,
        Mode::Auto => resolve_auto_mode(config),
    }
}

/// Implements the adaptive heuristic for [`Mode::Auto`].
fn resolve_auto_mode(config: &Config) -> Mode {
    use sysinfo::{MemoryRefreshKind, RefreshKind, System};

    // Sum original input file sizes (best-effort; unreadable files count as 0).
    let total_input_bytes: u64 = config
        .inputs
        .iter()
        .filter_map(|p| std::fs::metadata(p).ok())
        .map(|m| m.len())
        .sum();

    let sys = System::new_with_specifics(
        RefreshKind::nothing().with_memory(MemoryRefreshKind::everything()),
    );
    let available_bytes = sys.available_memory();

    // Use Disk when inputs exceed 50 % of available RAM. The conservative
    // threshold accounts for the hash-set overhead on top of the raw data.
    if available_bytes > 0 && total_input_bytes > available_bytes / 2 {
        Mode::Disk
    } else {
        Mode::Ram
    }
}

/// Scans `config.inputs` for rich-format files (PDF, EPUB), extracts their
/// text into temporary plain-text files, and returns a cloned `Config` whose
/// `inputs` list replaces every rich-format path with the corresponding
/// temp-file path.  Plain-text paths are forwarded unchanged.
///
/// Returns `(resolved_config, temp_files, failures)`.  The caller must hold
/// `temp_files` alive for as long as the resolved config's paths are in use.
/// Files that fail to extract are skipped; their `(path, error)` pairs are
/// returned so the caller can surface them as warnings.
fn resolve_rich_inputs<P: ProgressSink, C: CancelCheck>(
    config: &Config,
    progress: &P,
    cancel: &C,
) -> anyhow::Result<(Config, Vec<NamedTempFile>, Vec<(PathBuf, String)>, HashMap<PathBuf, PathBuf>)> {
    let total_rich = config
        .inputs
        .iter()
        .filter(|p| pdf_reader::is_pdf(p) || epub_reader::is_epub(p))
        .count();
    if total_rich == 0 {
        return Ok((config.clone(), Vec::new(), Vec::new(), HashMap::new()));
    }

    progress.on_event(ProgressEvent::Stage("ExtractingText"));

    let mut temp_files: Vec<NamedTempFile> = Vec::new();
    let mut resolved_inputs: Vec<PathBuf> = Vec::with_capacity(config.inputs.len());
    let mut failures: Vec<(PathBuf, String)> = Vec::new();
    // Maps temp-file path → original input path so per-file stats show the
    // real file name instead of a system temp path like ".tmpXgIdbM".
    let mut path_aliases: HashMap<PathBuf, PathBuf> = HashMap::new();
    let mut rich_done = 0usize;

    for path in &config.inputs {
        ensure_not_canceled(cancel)?;

        if pdf_reader::is_pdf(path) {
            let rich_index = rich_done + 1;
            progress.on_event(ProgressEvent::StageItemStarted {
                index: rich_index,
                total: total_rich,
                path: path.clone(),
            });
            match pdf_reader::pdf_to_temp_text(path) {
                Ok(tmp) => {
                    let temp_path = tmp.path().to_path_buf();
                    path_aliases.insert(temp_path.clone(), path.clone());
                    resolved_inputs.push(temp_path);
                    temp_files.push(tmp);
                }
                Err(err) => {
                    failures.push((path.clone(), format!("{err:#}")));
                }
            }
            rich_done = rich_index;
            progress.on_event(ProgressEvent::StageItemFinished {
                index: rich_done,
                total: total_rich,
            });
        } else if epub_reader::is_epub(path) {
            let rich_index = rich_done + 1;
            progress.on_event(ProgressEvent::StageItemStarted {
                index: rich_index,
                total: total_rich,
                path: path.clone(),
            });
            match epub_reader::epub_to_temp_text(path, cancel) {
                Ok(tmp) => {
                    let temp_path = tmp.path().to_path_buf();
                    path_aliases.insert(temp_path.clone(), path.clone());
                    resolved_inputs.push(temp_path);
                    temp_files.push(tmp);
                }
                Err(err) => {
                    failures.push((path.clone(), format!("{err:#}")));
                }
            }
            rich_done = rich_index;
            progress.on_event(ProgressEvent::StageItemFinished {
                index: rich_done,
                total: total_rich,
            });
        } else {
            resolved_inputs.push(path.clone());
        }
    }

    let mut resolved = config.clone();
    resolved.inputs = resolved_inputs;

    Ok((resolved, temp_files, failures, path_aliases))
}

fn run_ram<P: ProgressSink, C: CancelCheck>(
    config: &Config,
    progress: &P,
    cancel: &C,
    path_aliases: &HashMap<PathBuf, PathBuf>,
) -> anyhow::Result<Stats> {
    let started = Instant::now();
    progress.on_event(ProgressEvent::Stage("Tokenizing"));

    let mut store = match config.ordering {
        OutputOrdering::UnorderedFast => RamStore::new_unordered(),
        OutputOrdering::PreserveFirstSeen | OutputOrdering::Alphabetical => RamStore::new_stable(),
    };

    store.reserve(16 * 1024);

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

        // Per-file counters (only used when per_file_stats is enabled).
        let mut pf_tokens_seen: u64 = 0;
        let mut pf_duplicates: u64 = 0;
        let mut pf_unique_new: u64 = 0;
        let mut pf_filtered: u64 = 0;

        let file = File::open(path)?;
        let mut reader = LossyLineReader::new(BufReader::new(file));
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
                        progress.on_event(ProgressEvent::FilteredByLength(
                            stats.filtered_by_length,
                        ));
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
            // Use the original input path (pre-temp-extraction) when available
            // so the per-file breakdown shows the real file name, not ".tmpXXX".
            let display_path = path_aliases.get(path).cloned().unwrap_or_else(|| path.clone());
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
