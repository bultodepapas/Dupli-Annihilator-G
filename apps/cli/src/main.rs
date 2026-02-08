use anyhow::{anyhow, Context, Result};
use clap::{ArgAction, Parser, ValueEnum};
use dedupe_core::{
    run, Config, DiskAlphabeticalMode, Mode, NoProgress, OutputOrdering, ProgressEvent, ProgressSink,
};
use std::path::PathBuf;

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

#[derive(Debug, Default, Clone, Copy)]
struct ConsoleProgress {
    quiet: bool,
}

impl ProgressSink for ConsoleProgress {
    fn on_event(&self, event: ProgressEvent) {
        if self.quiet {
            return;
        }

        match event {
            ProgressEvent::Stage(stage) => eprintln!("[stage] {stage}"),
            ProgressEvent::FileStarted { index, total } => {
                eprintln!("[file] start {index}/{total}")
            }
            ProgressEvent::FileFinished { index, total } => {
                eprintln!("[file] done {index}/{total}")
            }
            ProgressEvent::TokensSeen(v) => eprintln!("[progress] tokens_seen={v}"),
            ProgressEvent::UniqueTokens(v) => eprintln!("[progress] unique_tokens={v}"),
            ProgressEvent::Duplicates(v) => eprintln!("[progress] duplicates={v}"),
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let output_separator = if cli.raw_separator {
        cli.separator.clone()
    } else {
        parse_escaped_separator(&cli.separator)
    };

    let cfg = Config {
        inputs: cli.inputs,
        output: cli.output,
        output_separator,
        mode: map_mode(cli.mode),
        ordering: map_ordering(cli.ordering),
        trim: cli.trim,
        drop_empty: cli.drop_empty,
        disk_buckets: cli.disk_buckets,
        disk_alphabetical_mode: map_disk_mode(cli.disk_alphabetical_mode),
        disk_run_bytes: cli.disk_run_bytes,
    };

    if cli.quiet {
        let stats = run(&cfg, NoProgress).context("engine run failed")?;
        println!(
            "done files={} tokens_seen={} unique={} duplicates={} elapsed_ms={}",
            stats.files,
            stats.tokens_seen,
            stats.unique_tokens,
            stats.duplicates,
            stats.elapsed.as_millis()
        );
    } else {
        let stats = run(&cfg, ConsoleProgress { quiet: false }).context("engine run failed")?;
        println!(
            "done files={} tokens_seen={} unique={} duplicates={} elapsed_ms={}",
            stats.files,
            stats.tokens_seen,
            stats.unique_tokens,
            stats.duplicates,
            stats.elapsed.as_millis()
        );
    }

    Ok(())
}

fn map_mode(m: CliMode) -> Mode {
    match m {
        CliMode::Auto => Mode::Auto,
        CliMode::Ram => Mode::Ram,
        CliMode::Disk => Mode::Disk,
    }
}

fn map_ordering(o: CliOrdering) -> OutputOrdering {
    match o {
        CliOrdering::PreserveFirstSeen => OutputOrdering::PreserveFirstSeen,
        CliOrdering::Alphabetical => OutputOrdering::Alphabetical,
        CliOrdering::UnorderedFast => OutputOrdering::UnorderedFast,
    }
}

fn map_disk_mode(m: CliDiskAlphabeticalMode) -> DiskAlphabeticalMode {
    match m {
        CliDiskAlphabeticalMode::FastBucketLocal => DiskAlphabeticalMode::FastBucketLocal,
        CliDiskAlphabeticalMode::GlobalPerfect => DiskAlphabeticalMode::GlobalPerfect,
    }
}

fn parse_escaped_separator(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }

        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => {
                if matches!(chars.peek(), Some('n')) {
                    chars.next();
                    out.push('\r');
                    out.push('\n');
                } else {
                    out.push('\r');
                }
            }
            Some('f') => out.push('\u{000C}'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }

    out
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
    use super::{parse_escaped_separator, parse_size_bytes};

    #[test]
    fn parse_separator_escapes() {
        assert_eq!(parse_escaped_separator("\\n"), "\n");
        assert_eq!(parse_escaped_separator(",\\n"), ",\n");
        assert_eq!(parse_escaped_separator("\\t"), "\t");
        assert_eq!(parse_escaped_separator("\\r\\n"), "\r\n");
        assert_eq!(parse_escaped_separator("\\f"), "\u{000C}");
        assert_eq!(parse_escaped_separator("\\\\"), "\\");
        assert_eq!(parse_escaped_separator("\\x"), "\\x");
    }

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
