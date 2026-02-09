use anyhow::{anyhow, Context, Result};
use clap::{ArgAction, Parser, ValueEnum};
use dedupe_backend::{
    ApiDiskAlphabeticalMode, ApiMode, ApiOrdering, BackendJobEvent, BackendService,
    CancelJobRequest, StartJobConfig, StartJobRequest,
};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering as AtomicOrdering},
    Arc,
};
use std::time::Duration;

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliMode {
    Auto,
    Ram,
    Disk,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliOrdering {
    PreserveFirstSeen,
    Alphabetical,
    UnorderedFast,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliDiskAlphabeticalMode {
    FastBucketLocal,
    GlobalPerfect,
}

#[derive(Debug, Parser)]
#[command(name = "dedupe-cli")]
#[command(about = "Merge + dedupe tokens from text files (Rust core)")]
struct Cli {
    #[arg(short, long = "input", required = true, num_args = 1..)]
    inputs: Vec<PathBuf>,

    #[arg(short, long)]
    output: PathBuf,

    #[arg(long, action = ArgAction::SetTrue)]
    overwrite: bool,

    #[arg(long, value_enum, default_value_t = CliMode::Ram)]
    mode: CliMode,

    #[arg(long, value_enum, default_value_t = CliOrdering::PreserveFirstSeen)]
    ordering: CliOrdering,

    #[arg(long = "disk-alphabetical-mode", value_enum, default_value_t = CliDiskAlphabeticalMode::FastBucketLocal)]
    disk_alphabetical_mode: CliDiskAlphabeticalMode,

    #[arg(long = "separator", default_value = "\\n")]
    separator: String,

    #[arg(long = "raw-separator", action = ArgAction::SetTrue)]
    raw_separator: bool,

    #[arg(long = "trim", default_value_t = true, action = ArgAction::Set)]
    trim: bool,

    #[arg(long = "drop-empty", default_value_t = true, action = ArgAction::Set)]
    drop_empty: bool,

    #[arg(long = "disk-buckets", default_value_t = 256)]
    disk_buckets: usize,

    #[arg(long = "disk-run-size", default_value = "256MB", value_parser = parse_size_bytes)]
    disk_run_bytes: usize,

    #[arg(long, action = ArgAction::SetTrue)]
    quiet: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let backend = BackendService::new();
    let start = backend
        .start_job(StartJobRequest {
            config: StartJobConfig {
                inputs: cli
                    .inputs
                    .iter()
                    .map(|p| p.to_string_lossy().to_string())
                    .collect(),
                output: cli.output.to_string_lossy().to_string(),
                allow_overwrite: cli.overwrite,
                output_separator: cli.separator.clone(),
                interpret_separator_escapes: !cli.raw_separator,
                mode: map_mode(cli.mode),
                ordering: map_ordering(cli.ordering),
                trim: cli.trim,
                drop_empty: cli.drop_empty,
                disk_buckets: cli.disk_buckets,
                disk_alphabetical_mode: map_disk_mode(cli.disk_alphabetical_mode),
                disk_run_bytes: cli.disk_run_bytes,
            },
        })
        .map_err(|e| anyhow!("failed to start job [{}]: {}", e.category, e.message))?;
    let job_id = start.job_id;

    let cancel_requested = Arc::new(AtomicBool::new(false));
    {
        let cancel_requested = Arc::clone(&cancel_requested);
        ctrlc::set_handler(move || {
            cancel_requested.store(true, AtomicOrdering::Release);
        })
        .context("failed to install Ctrl+C handler")?;
    }

    let mut sent_cancel = false;
    loop {
        if cancel_requested.load(AtomicOrdering::Acquire) && !sent_cancel {
            sent_cancel = backend.cancel_job(CancelJobRequest { job_id }).acknowledged;
            if sent_cancel && !cli.quiet {
                eprintln!("[signal] cancellation requested");
            }
        }

        let Some(event) = backend.next_event_timeout(Duration::from_millis(200)) else {
            continue;
        };

        match event {
            BackendJobEvent::Started { .. } => {
                if !cli.quiet {
                    eprintln!("[job] started id={job_id}");
                }
            }
            BackendJobEvent::Stage { stage, .. } => {
                if !cli.quiet {
                    eprintln!("[stage] {stage}");
                }
            }
            BackendJobEvent::Progress {
                files_done,
                files_total,
                tokens_seen,
                unique_tokens,
                duplicates,
                throughput_tps,
                elapsed_ms,
                eta_ms,
                ..
            } => {
                if !cli.quiet {
                    eprintln!(
                        "[progress] files={files_done}/{files_total} tokens={tokens_seen} unique={unique_tokens} dup={duplicates} tps={throughput_tps} elapsed_ms={elapsed_ms} eta_ms={}",
                        eta_ms.map(|v| v.to_string()).unwrap_or_else(|| "-".to_string())
                    );
                }
            }
            BackendJobEvent::Done { stats, .. } => {
                println!(
                    "done files={} tokens_seen={} unique={} duplicates={} elapsed_ms={}",
                    stats.files,
                    stats.tokens_seen,
                    stats.unique_tokens,
                    stats.duplicates,
                    stats.elapsed_ms
                );
                break;
            }
            BackendJobEvent::Error { message, .. } => {
                return Err(anyhow!("engine run failed: {message}"));
            }
            BackendJobEvent::Canceled { .. } => {
                return Err(anyhow!("job canceled"));
            }
            BackendJobEvent::Summary { summary, .. } => {
                if !cli.quiet {
                    eprintln!(
                        "[summary] status={:?} reduction_pct={} uniq_pct={} output_bytes={}",
                        summary.status,
                        summary.reduction_pct,
                        summary.uniq_pct,
                        summary.output_bytes
                    );
                }
            }
        }
    }

    Ok(())
}

fn map_mode(m: CliMode) -> ApiMode {
    match m {
        CliMode::Auto => ApiMode::Auto,
        CliMode::Ram => ApiMode::Ram,
        CliMode::Disk => ApiMode::Disk,
    }
}

fn map_ordering(o: CliOrdering) -> ApiOrdering {
    match o {
        CliOrdering::PreserveFirstSeen => ApiOrdering::PreserveFirstSeen,
        CliOrdering::Alphabetical => ApiOrdering::Alphabetical,
        CliOrdering::UnorderedFast => ApiOrdering::UnorderedFast,
    }
}

fn map_disk_mode(m: CliDiskAlphabeticalMode) -> ApiDiskAlphabeticalMode {
    match m {
        CliDiskAlphabeticalMode::FastBucketLocal => ApiDiskAlphabeticalMode::FastBucketLocal,
        CliDiskAlphabeticalMode::GlobalPerfect => ApiDiskAlphabeticalMode::GlobalPerfect,
    }
}

fn parse_size_bytes(raw: &str) -> Result<usize> {
    let s = raw.trim();
    if s.is_empty() {
        return Err(anyhow!("size cannot be empty"));
    }

    let mut num_end = 0;
    for (i, ch) in s.char_indices() {
        if ch.is_ascii_digit() {
            num_end = i + ch.len_utf8();
        } else {
            break;
        }
    }

    if num_end == 0 {
        return Err(anyhow!("size must start with digits: '{s}'"));
    }

    let number: u64 = s[..num_end]
        .parse()
        .with_context(|| format!("invalid size number in '{s}'"))?;

    let unit = s[num_end..].trim().to_ascii_uppercase();
    let mult: u64 = match unit.as_str() {
        "" | "B" => 1,
        "K" | "KB" => 1024,
        "M" | "MB" => 1024 * 1024,
        "G" | "GB" => 1024 * 1024 * 1024,
        _ => return Err(anyhow!("invalid size unit in '{s}' (use B/KB/MB/GB)")),
    };

    let bytes = number
        .checked_mul(mult)
        .ok_or_else(|| anyhow!("size overflow for '{s}'"))?;

    usize::try_from(bytes).map_err(|_| anyhow!("size too large for this platform: '{s}'"))
}

#[cfg(test)]
mod tests {
    use super::parse_size_bytes;

    #[test]
    fn parse_size_units() {
        assert_eq!(parse_size_bytes("42").expect("size"), 42);
        assert_eq!(parse_size_bytes("1KB").expect("size"), 1024);
        assert_eq!(parse_size_bytes("2MB").expect("size"), 2 * 1024 * 1024);
        assert_eq!(
            parse_size_bytes("3GB").expect("size"),
            3 * 1024 * 1024 * 1024usize
        );
        assert!(parse_size_bytes("abc").is_err());
        assert!(parse_size_bytes("10XB").is_err());
    }
}
