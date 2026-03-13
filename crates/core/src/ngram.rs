use crate::{
    cancel::{ensure_not_canceled, CancelCheck},
    config::{Config, OutputOrdering},
    dedupe_ram::RamStore,
    progress::{ProgressEvent, ProgressSink},
    text_line_reader::LossyLineReader,
    token_iter::TokenIter,
    writer::OutputWriter,
};
use std::collections::VecDeque;
use std::fs::File;
use std::io::BufReader;
use std::time::Instant;

/// Statistics returned by [`ngram_extract`].
#[derive(Debug, Clone)]
pub struct NgramStats {
    /// Individual source tokens processed across all input files.
    pub tokens_seen: u64,
    /// Total n-grams generated before deduplication.
    pub ngrams_seen: u64,
    /// Unique n-grams written to the output file.
    pub unique_ngrams: u64,
    /// Wall-clock time for the entire operation.
    pub elapsed: std::time::Duration,
}

/// Extracts all unique n-grams of size `n` from `config.inputs` and writes
/// them to `config.output` using the configured separator and ordering.
///
/// An n-gram is `n` consecutive tokens joined by a single space, e.g.
/// `"machine learning"` for `n = 2`.  Tokens are produced by the same
/// [`TokenIter`] used by the dedup engine (splits on whitespace, `,`, `;`).
///
/// # Window semantics
///
/// * The sliding window of size `n` advances one token at a time.
/// * The window is **reset at every line boundary** so n-grams never span
///   two source lines — e.g. the last token of line 1 and the first token
///   of line 2 are never combined.
/// * Tokens that are dropped (empty after trim, or filtered by the length
///   filter) also reset the window, preventing n-grams with logical gaps.
///
/// # Deduplication
///
/// Identical n-gram strings are deduplicated in RAM.  Output ordering
/// respects `config.ordering`:
/// * `PreserveFirstSeen` — first occurrence order.
/// * `Alphabetical` — lexicographic sort.
/// * `UnorderedFast` — no ordering guarantee (fastest).
///
/// # Constraints
///
/// * `n` must be in `2..=20`.
/// * All [`Config::validate`] constraints apply (non-empty inputs and output,
///   non-empty separator).
///
/// # Zero impact on the core dedup engine
///
/// Runs entirely outside the [`run`] / [`run_with_control`] paths.
///
/// [`run`]: crate::run
/// [`run_with_control`]: crate::run_with_control
pub fn ngram_extract<P: ProgressSink, C: CancelCheck>(
    n: usize,
    config: &Config,
    progress: &P,
    cancel: &C,
) -> anyhow::Result<NgramStats> {
    anyhow::ensure!(
        n >= 2 && n <= 20,
        "n must be between 2 and 20, got {n}"
    );
    config.validate()?;

    let started = Instant::now();
    let total_files = config.inputs.len();

    progress.on_event(ProgressEvent::Stage("NgramExtract"));

    // Use a stable store for ordering modes that care about insertion order;
    // unordered is fine for UnorderedFast.
    let mut store = match config.ordering {
        OutputOrdering::UnorderedFast => RamStore::new_unordered(),
        OutputOrdering::PreserveFirstSeen | OutputOrdering::Alphabetical => RamStore::new_stable(),
    };
    store.reserve(16 * 1024);

    let mut tokens_seen: u64 = 0;
    let mut ngrams_seen: u64 = 0;
    // Sliding window — fixed capacity `n`.
    let mut window: VecDeque<Box<str>> = VecDeque::with_capacity(n);
    // Buffer reused across reads — LossyLineReader clears it on each call.
    let mut line = String::new();
    // Reusable string for building gram candidates without per-gram allocation.
    let mut gram_buf = String::new();

    for (idx, path) in config.inputs.iter().enumerate() {
        ensure_not_canceled(cancel)?;
        progress.on_event(ProgressEvent::FileStarted {
            index: idx + 1,
            total: total_files,
        });

        let file = File::open(path)?;
        let mut reader = LossyLineReader::new(BufReader::new(file));

        loop {
            ensure_not_canceled(cancel)?;
            let bytes_read = reader.read_line(&mut line)?;
            if bytes_read == 0 {
                break;
            }

            for raw in TokenIter::new(&line) {
                let token = if config.trim { raw.trim() } else { raw };

                // Dropped tokens break the n-gram sequence — reset the window
                // to avoid producing grams with logical gaps.
                if config.drop_empty && token.is_empty() {
                    window.clear();
                    continue;
                }
                if config.should_drop_by_length(token) {
                    window.clear();
                    continue;
                }

                window.push_back(token.into());
                tokens_seen += 1;

                if window.len() == n {
                    // Build the gram string into the reusable buffer.
                    gram_buf.clear();
                    // Reserve: sum of token lengths + (n-1) spaces.
                    let cap: usize = window.iter().map(|t| t.len()).sum::<usize>() + n - 1;
                    gram_buf.reserve(cap.saturating_sub(gram_buf.capacity()));

                    for (i, tok) in window.iter().enumerate() {
                        if i > 0 {
                            gram_buf.push(' ');
                        }
                        gram_buf.push_str(tok);
                    }

                    ngrams_seen += 1;
                    store.insert(&gram_buf);
                    window.pop_front();
                }

                if tokens_seen % 8_192 == 0 {
                    ensure_not_canceled(cancel)?;
                }
            }

            // N-grams must not span line boundaries.
            window.clear();
        }

        progress.on_event(ProgressEvent::FileFinished {
            index: idx + 1,
            total: total_files,
        });
    }

    // Collect unique grams and apply ordering.
    let mut grams: Vec<Box<str>> = store.into_tokens();
    let unique_ngrams = grams.len() as u64;

    if matches!(config.ordering, OutputOrdering::Alphabetical) {
        progress.on_event(ProgressEvent::Stage("Sorting"));
        grams.sort_unstable();
    }

    // Write output.
    progress.on_event(ProgressEvent::Stage("WritingOutput"));
    let mut writer = OutputWriter::create(&config.output, config.output_separator.clone())?;
    for gram in &grams {
        writer.write_token(gram)?;
    }
    writer.finish()?;

    Ok(NgramStats {
        tokens_seen,
        ngrams_seen,
        unique_ngrams,
        elapsed: started.elapsed(),
    })
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{cancel::NoCancel, config::Config, progress::NoProgress};
    use std::io::Write;
    use tempfile::NamedTempFile;

    // ── helpers ───────────────────────────────────────────────────────────────

    fn write_temp(content: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f.flush().unwrap();
        f
    }

    fn read_output(path: &std::path::Path) -> Vec<String> {
        let content = std::fs::read_to_string(path).unwrap();
        if content.is_empty() {
            vec![]
        } else {
            content.split('\n').map(str::to_string).collect()
        }
    }

    fn cfg(inputs: Vec<std::path::PathBuf>, output: std::path::PathBuf) -> Config {
        Config {
            inputs,
            output,
            trim: true,
            drop_empty: true,
            ..Config::default()
        }
    }

    // ── constraint validation ─────────────────────────────────────────────────

    #[test]
    fn n_below_2_returns_error() {
        let f = write_temp("hello world\n");
        let out = NamedTempFile::new().unwrap();
        let c = cfg(vec![f.path().into()], out.path().into());
        assert!(ngram_extract(1, &c, &NoProgress, &NoCancel).is_err());
    }

    #[test]
    fn n_above_20_returns_error() {
        let f = write_temp("hello world\n");
        let out = NamedTempFile::new().unwrap();
        let c = cfg(vec![f.path().into()], out.path().into());
        assert!(ngram_extract(21, &c, &NoProgress, &NoCancel).is_err());
    }

    // ── bigrams ───────────────────────────────────────────────────────────────

    #[test]
    fn bigrams_from_simple_line() {
        let f = write_temp("a b c\n");
        let out = NamedTempFile::new().unwrap();
        let c = cfg(vec![f.path().into()], out.path().into());
        let stats = ngram_extract(2, &c, &NoProgress, &NoCancel).unwrap();
        assert_eq!(stats.tokens_seen, 3);
        assert_eq!(stats.ngrams_seen, 2);
        assert_eq!(stats.unique_ngrams, 2);
        let mut lines = read_output(out.path());
        lines.sort();
        assert_eq!(lines, vec!["a b", "b c"]);
    }

    #[test]
    fn single_token_line_produces_no_bigrams() {
        let f = write_temp("hello\n");
        let out = NamedTempFile::new().unwrap();
        let c = cfg(vec![f.path().into()], out.path().into());
        let stats = ngram_extract(2, &c, &NoProgress, &NoCancel).unwrap();
        assert_eq!(stats.ngrams_seen, 0);
        assert_eq!(stats.unique_ngrams, 0);
        assert!(read_output(out.path()).is_empty());
    }

    // ── trigrams ──────────────────────────────────────────────────────────────

    #[test]
    fn trigrams_from_four_token_line() {
        let f = write_temp("a b c d\n");
        let out = NamedTempFile::new().unwrap();
        let c = cfg(vec![f.path().into()], out.path().into());
        let stats = ngram_extract(3, &c, &NoProgress, &NoCancel).unwrap();
        assert_eq!(stats.ngrams_seen, 2);
        let mut lines = read_output(out.path());
        lines.sort();
        assert_eq!(lines, vec!["a b c", "b c d"]);
    }

    // ── line boundary ─────────────────────────────────────────────────────────

    #[test]
    fn window_resets_at_line_boundary() {
        // "a b" on line 1, "c d" on line 2.
        // Valid bigrams: "a b" and "c d" — NOT "b c".
        let f = write_temp("a b\nc d\n");
        let out = NamedTempFile::new().unwrap();
        let c = cfg(vec![f.path().into()], out.path().into());
        ngram_extract(2, &c, &NoProgress, &NoCancel).unwrap();
        let mut lines = read_output(out.path());
        lines.sort();
        assert_eq!(lines, vec!["a b", "c d"]);
        assert!(!lines.contains(&"b c".to_string()));
    }

    #[test]
    fn window_resets_when_token_is_dropped() {
        // Use a 2-char separator token that gets dropped, while the grams we
        // want to preserve are 3-char tokens.
        let f = write_temp("xx ant bee xx cat dog\n");
        let out = NamedTempFile::new().unwrap();
        let mut c = cfg(vec![f.path().into()], out.path().into());
        c.drop_length_min = Some(2);
        c.drop_length_max = Some(2);
        ngram_extract(2, &c, &NoProgress, &NoCancel).unwrap();
        let mut lines = read_output(out.path());
        lines.sort();
        assert_eq!(lines, vec!["ant bee", "cat dog"]);
    }

    // ── deduplication ─────────────────────────────────────────────────────────

    #[test]
    fn repeated_ngrams_are_deduplicated() {
        let f = write_temp("a b\na b\na b\n");
        let out = NamedTempFile::new().unwrap();
        let c = cfg(vec![f.path().into()], out.path().into());
        let stats = ngram_extract(2, &c, &NoProgress, &NoCancel).unwrap();
        assert_eq!(stats.ngrams_seen, 3); // generated 3 times
        assert_eq!(stats.unique_ngrams, 1); // only one unique
        assert_eq!(read_output(out.path()), vec!["a b"]);
    }

    // ── ordering ─────────────────────────────────────────────────────────────

    #[test]
    fn alphabetical_ordering_applied() {
        let f = write_temp("c d\na b\nb c\n");
        let out = NamedTempFile::new().unwrap();
        let mut c = cfg(vec![f.path().into()], out.path().into());
        c.ordering = OutputOrdering::Alphabetical;
        ngram_extract(2, &c, &NoProgress, &NoCancel).unwrap();
        assert_eq!(read_output(out.path()), vec!["a b", "b c", "c d"]);
    }

    #[test]
    fn preserve_first_seen_ordering() {
        let f = write_temp("c d\na b\nb c\n");
        let out = NamedTempFile::new().unwrap();
        let mut c = cfg(vec![f.path().into()], out.path().into());
        c.ordering = OutputOrdering::PreserveFirstSeen;
        ngram_extract(2, &c, &NoProgress, &NoCancel).unwrap();
        // First bigram seen is "c d", then "a b", then "b c".
        assert_eq!(read_output(out.path()), vec!["c d", "a b", "b c"]);
    }

    // ── edge cases ────────────────────────────────────────────────────────────

    #[test]
    fn empty_file_produces_empty_output() {
        let f = write_temp("");
        let out = NamedTempFile::new().unwrap();
        let c = cfg(vec![f.path().into()], out.path().into());
        let stats = ngram_extract(2, &c, &NoProgress, &NoCancel).unwrap();
        assert_eq!(stats.tokens_seen, 0);
        assert_eq!(stats.unique_ngrams, 0);
        assert!(read_output(out.path()).is_empty());
    }

    #[test]
    fn multiple_files_accumulated() {
        let f1 = write_temp("a b\n");
        let f2 = write_temp("c d\n");
        let out = NamedTempFile::new().unwrap();
        let c = cfg(vec![f1.path().into(), f2.path().into()], out.path().into());
        let stats = ngram_extract(2, &c, &NoProgress, &NoCancel).unwrap();
        assert_eq!(stats.unique_ngrams, 2);
        let mut lines = read_output(out.path());
        lines.sort();
        assert_eq!(lines, vec!["a b", "c d"]);
    }

    #[test]
    fn larger_n_produces_correct_grams() {
        // 5-gram from a 6-token line → 2 grams.
        let f = write_temp("a b c d e f\n");
        let out = NamedTempFile::new().unwrap();
        let c = cfg(vec![f.path().into()], out.path().into());
        let stats = ngram_extract(5, &c, &NoProgress, &NoCancel).unwrap();
        assert_eq!(stats.ngrams_seen, 2);
        let mut lines = read_output(out.path());
        lines.sort();
        assert_eq!(lines, vec!["a b c d e", "b c d e f"]);
    }

    #[test]
    fn comma_separated_tokens_split_correctly() {
        // TokenIter splits on commas — CSV-style input.
        let f = write_temp("hello,world,foo\n");
        let out = NamedTempFile::new().unwrap();
        let c = cfg(vec![f.path().into()], out.path().into());
        ngram_extract(2, &c, &NoProgress, &NoCancel).unwrap();
        let mut lines = read_output(out.path());
        lines.sort();
        assert_eq!(lines, vec!["hello world", "world foo"]);
    }

    // ── cancellation ─────────────────────────────────────────────────────────

    #[test]
    fn cancellation_returns_canceled_error() {
        use crate::cancel::CancellationToken;
        let tok = CancellationToken::new();
        tok.cancel();
        // Need enough tokens to hit the 8_192 check.  Use a large file.
        let content: String = (0..1000u32)
            .map(|i| format!("tok{i} "))
            .collect::<Vec<_>>()
            .join("")
            + "\n";
        let content = content.repeat(10);
        let f = write_temp(&content);
        let out = NamedTempFile::new().unwrap();
        let c = cfg(vec![f.path().into()], out.path().into());
        let err = ngram_extract(2, &c, &NoProgress, &tok).unwrap_err();
        assert!(crate::cancel::is_canceled_error(&err));
    }
}
