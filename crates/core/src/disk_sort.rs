use crate::{
    cancel::{ensure_not_canceled, CancelCheck},
    config::Config,
    progress::{ProgressEvent, ProgressSink},
    stats::Stats,
    text_line_reader::LossyLineReader,
    token_iter::TokenIter,
    writer::OutputWriter,
};
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

#[derive(Debug)]
struct RunFile {
    path: PathBuf,
}

pub fn external_sort_global<P: ProgressSink>(
    config: &Config,
    progress: &P,
    stats: &mut Stats,
    temp_dir: &Path,
    cancel: &impl CancelCheck,
) -> anyhow::Result<()> {
    progress.on_event(ProgressEvent::Stage("GeneratingRuns"));
    let runs = generate_runs(config, progress, stats, temp_dir, cancel)?;

    progress.on_event(ProgressEvent::Stage("MergingRuns"));
    merge_runs_to_output(config, stats, &runs, cancel)?;
    Ok(())
}

fn generate_runs<P: ProgressSink, C: CancelCheck>(
    config: &Config,
    progress: &P,
    stats: &mut Stats,
    temp_dir: &Path,
    cancel: &C,
) -> anyhow::Result<Vec<RunFile>> {
    let mut runs = Vec::new();
    let mut buf = Vec::<String>::new();
    let mut bytes_acc: usize = 0;

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
                if config.should_drop_by_length(token) {
                    stats.filtered_by_length += 1;
                    continue;
                }

                bytes_acc += token.len() + 1;
                buf.push(token.to_string());

                if bytes_acc >= config.disk_run_bytes {
                    flush_run(&mut runs, &mut buf, &mut bytes_acc, temp_dir, stats)?;
                }
            }
        }

        progress.on_event(ProgressEvent::FileFinished {
            index: idx + 1,
            total: config.inputs.len(),
        });
    }

    if !buf.is_empty() {
        flush_run(&mut runs, &mut buf, &mut bytes_acc, temp_dir, stats)?;
    }

    Ok(runs)
}

fn flush_run(
    runs: &mut Vec<RunFile>,
    buf: &mut Vec<String>,
    bytes_acc: &mut usize,
    temp_dir: &Path,
    stats: &mut Stats,
) -> anyhow::Result<()> {
    buf.sort_unstable();
    let before = buf.len();
    buf.dedup();
    let after = buf.len();
    stats.duplicates += (before - after) as u64;

    let run_idx = runs.len();
    let path = temp_dir.join(format!("run_{run_idx:05}.txt"));
    let file = File::create(&path)?;
    let mut writer = BufWriter::new(file);

    for t in buf.iter() {
        writer.write_all(t.as_bytes())?;
        writer.write_all(b"\n")?;
    }
    writer.flush()?;

    runs.push(RunFile { path });
    buf.clear();
    *bytes_acc = 0;

    Ok(())
}

fn merge_runs_to_output<C: CancelCheck>(
    config: &Config,
    stats: &mut Stats,
    runs: &[RunFile],
    cancel: &C,
) -> anyhow::Result<()> {
    let mut readers = Vec::with_capacity(runs.len());
    for run in runs {
        readers.push(BufReader::new(File::open(&run.path)?));
    }

    let mut heap: BinaryHeap<Reverse<(String, usize)>> = BinaryHeap::new();

    for (i, reader) in readers.iter_mut().enumerate() {
        let mut line = String::new();
        if reader.read_line(&mut line)? > 0 {
            let token = line.trim_end_matches(&['\n', '\r'][..]).to_string();
            heap.push(Reverse((token, i)));
        }
    }

    let mut out = OutputWriter::create(&config.output, config.output_separator.clone())?;
    let mut last_written: Option<String> = None;
    let mut merged: u64 = 0;

    while let Some(Reverse((token, run_id))) = heap.pop() {
        merged += 1;
        if merged % 8_192 == 0 {
            ensure_not_canceled(cancel)?;
        }
        if last_written.as_deref() != Some(token.as_str()) {
            out.write_token(&token)?;
            stats.unique_tokens += 1;
            last_written = Some(token.clone());
        } else {
            stats.duplicates += 1;
        }

        let reader = &mut readers[run_id];
        let mut line = String::new();
        if reader.read_line(&mut line)? > 0 {
            let next = line.trim_end_matches(&['\n', '\r'][..]).to_string();
            heap.push(Reverse((next, run_id)));
        }
    }

    out.finish()?;
    Ok(())
}
