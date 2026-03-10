use crate::{
    cancel::{ensure_not_canceled, CancelCheck, NoCancel},
    config::{Config, DiskAlphabeticalMode, Mode, OutputOrdering},
    dedupe_ram::RamStore,
    disk::DiskBuckets,
    disk_sort,
    pdf_reader,
    progress::{ProgressEvent, ProgressSink},
    stats::Stats,
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

    // Resolve any PDF inputs to temporary plain-text files before the engine
    // loop runs.  The returned `_temp_files` vec keeps the NamedTempFiles alive
    // for the entire duration of the job; they are deleted when it drops.
    let (resolved, _temp_files) = resolve_pdf_inputs(config, &progress, &cancel)?;

    match resolved.mode {
        Mode::Ram => run_ram(&resolved, &progress, &cancel),
        Mode::Disk => run_disk(&resolved, &progress, &cancel),
        Mode::Auto => run_ram(&resolved, &progress, &cancel),
    }
}

/// Scans `config.inputs` for `.pdf` files, extracts their text into temporary
/// files, and returns a cloned `Config` whose `inputs` list replaces every PDF
/// path with the corresponding temp-file path.  Non-PDF paths are forwarded
/// unchanged.
///
/// Returns `(resolved_config, temp_files)`.  The caller must hold `temp_files`
/// alive for as long as the resolved config's paths are in use.
fn resolve_pdf_inputs<P: ProgressSink, C: CancelCheck>(
    config: &Config,
    progress: &P,
    cancel: &C,
) -> anyhow::Result<(Config, Vec<NamedTempFile>)> {
    let pdf_count = config
        .inputs
        .iter()
        .filter(|p| pdf_reader::is_pdf(p))
        .count();

    if pdf_count == 0 {
        return Ok((config.clone(), Vec::new()));
    }

    progress.on_event(ProgressEvent::Stage("ExtractingPdf"));

    let mut temp_files: Vec<NamedTempFile> = Vec::with_capacity(pdf_count);
    let mut resolved_inputs: Vec<PathBuf> = Vec::with_capacity(config.inputs.len());

    for path in &config.inputs {
        ensure_not_canceled(cancel)?;

        if pdf_reader::is_pdf(path) {
            let tmp = pdf_reader::pdf_to_temp_text(path)?;
            resolved_inputs.push(tmp.path().to_path_buf());
            temp_files.push(tmp);
        } else {
            resolved_inputs.push(path.clone());
        }
    }

    let mut resolved = config.clone();
    resolved.inputs = resolved_inputs;

    Ok((resolved, temp_files))
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
        ..Default::default()
    };

    for (idx, path) in config.inputs.iter().enumerate() {
        ensure_not_canceled(cancel)?;
        progress.on_event(ProgressEvent::FileStarted {
            index: idx + 1,
            total: config.inputs.len(),
        });

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

                if store.insert(token) {
                    stats.unique_tokens += 1;
                    if stats.unique_tokens % 100_000 == 0 {
                        progress.on_event(ProgressEvent::UniqueTokens(stats.unique_tokens));
                    }
                } else {
                    stats.duplicates += 1;
                    if stats.duplicates % 100_000 == 0 {
                        progress.on_event(ProgressEvent::Duplicates(stats.duplicates));
                    }
                }
            }
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
