use crate::{
    cancel::{ensure_not_canceled, CancelCheck, NoCancel},
    config::{Config, DiskAlphabeticalMode, Mode, OutputOrdering},
    dedupe_ram::RamStore,
    disk::DiskBuckets,
    disk_sort,
    epub_reader,
    pdf_reader,
    progress::{ProgressEvent, ProgressSink},
    stats::{FileStats, Stats},
    text_line_reader::LossyLineReader,
    token_iter::TokenIter,
    writer::OutputWriter,
};
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
    let (resolved, _temp_files, failed_pdfs) = resolve_rich_inputs(config, &progress, &cancel)?;

    let mut stats = match resolved.mode {
        Mode::Ram => run_ram(&resolved, &progress, &cancel),
        Mode::Disk => run_disk(&resolved, &progress, &cancel),
        Mode::Auto => run_ram(&resolved, &progress, &cancel),
    }?;

    stats.failed_pdfs = failed_pdfs;
    Ok(stats)
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
) -> anyhow::Result<(Config, Vec<NamedTempFile>, Vec<(PathBuf, String)>)> {
    let rich_count = config
        .inputs
        .iter()
        .filter(|p| pdf_reader::is_pdf(p) || epub_reader::is_epub(p))
        .count();

    if rich_count == 0 {
        return Ok((config.clone(), Vec::new(), Vec::new()));
    }

    progress.on_event(ProgressEvent::Stage("ExtractingText"));

    let mut temp_files: Vec<NamedTempFile> = Vec::with_capacity(rich_count);
    let mut resolved_inputs: Vec<PathBuf> = Vec::with_capacity(config.inputs.len());
    let mut failures: Vec<(PathBuf, String)> = Vec::new();

    for path in &config.inputs {
        ensure_not_canceled(cancel)?;

        if pdf_reader::is_pdf(path) {
            match pdf_reader::pdf_to_temp_text(path) {
                Ok(tmp) => {
                    resolved_inputs.push(tmp.path().to_path_buf());
                    temp_files.push(tmp);
                }
                Err(err) => {
                    failures.push((path.clone(), format!("{err:#}")));
                }
            }
        } else if epub_reader::is_epub(path) {
            match epub_reader::epub_to_temp_text(path) {
                Ok(tmp) => {
                    resolved_inputs.push(tmp.path().to_path_buf());
                    temp_files.push(tmp);
                }
                Err(err) => {
                    failures.push((path.clone(), format!("{err:#}")));
                }
            }
        } else {
            resolved_inputs.push(path.clone());
        }
    }

    let mut resolved = config.clone();
    resolved.inputs = resolved_inputs;

    Ok((resolved, temp_files, failures))
}

fn run_ram<P: ProgressSink, C: CancelCheck>(
    config: &Config,
    progress: &P,
    cancel: &C,
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
            let file_bytes = std::fs::metadata(path).map(|m| m.len()).ok();
            per_file.push(FileStats {
                path: path.clone(),
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
                let mut buckets = DiskBuckets::new(config.disk_buckets)?;
                buckets.partition_inputs(config, progress, &mut stats, cancel)?;
                buckets.reduce_to_output(config, progress, &mut stats, cancel)?;
            }
            DiskAlphabeticalMode::GlobalPerfect => {
                let temp = tempfile::tempdir()?;
                disk_sort::external_sort_global(config, progress, &mut stats, temp.path(), cancel)?;
            }
        }
    } else {
        let mut buckets = DiskBuckets::new(config.disk_buckets)?;
        buckets.partition_inputs(config, progress, &mut stats, cancel)?;
        buckets.reduce_to_output(config, progress, &mut stats, cancel)?;
    }

    progress.on_event(ProgressEvent::Stage("Finalizing"));
    stats.elapsed = started.elapsed();
    Ok(stats)
}
