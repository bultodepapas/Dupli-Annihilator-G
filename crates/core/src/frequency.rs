use crate::{
    cancel::{ensure_not_canceled, CancelCheck},
    config::Config,
    progress::{ProgressEvent, ProgressSink},
    text_line_reader::LossyLineReader,
    token_iter::TokenIter,
};
use ahash::RandomState;
use hashbrown::HashMap;
use std::fs::File;
use std::io::BufReader;

/// Reads all inputs in `config`, tokenises them with the same rules as the
/// deduplication engine, and returns every token together with how many times
/// it appeared across all input files.
///
/// The returned `Vec` is sorted **descending by count**; ties are broken by
/// ascending token text so that the result is fully deterministic.
///
/// # Behaviour
///
/// - Does **not** call [`Config::validate`] because no output file is written.
///   An empty `config.inputs` is accepted and returns `Ok(vec![])`.
/// - Respects `config.trim`, `config.drop_empty`, and the length-filter fields
///   (`drop_length_min` / `drop_length_max`) in the same order as [`run`].
/// - Cancellation is checked once per line and additionally every 8 192
///   tokens; [`ProgressEvent`] is emitted every 100 000 tokens and at file
///   start / finish boundaries.
///
/// [`run`]: crate::run
pub fn token_frequency<P: ProgressSink, C: CancelCheck>(
    config: &Config,
    progress: &P,
    cancel: &C,
) -> anyhow::Result<Vec<(Box<str>, u64)>> {
    if config.inputs.is_empty() {
        return Ok(Vec::new());
    }

    progress.on_event(ProgressEvent::Stage("FrequencyAnalysis"));

    // Same hasher strategy as RamStore / WordChecker: AHash via RandomState.
    let mut counts: HashMap<Box<str>, u64, RandomState> = HashMap::with_hasher(RandomState::new());
    // Pre-allocate roughly the same initial capacity as RamStore (engine.rs:179).
    counts.reserve(16 * 1024);

    let mut tokens_seen: u64 = 0;
    // Declared outside the file loop so the allocation is reused across files.
    // LossyLineReader::read_line calls out.clear() on entry, so no manual reset needed.
    let mut line = String::new();

    for (idx, path) in config.inputs.iter().enumerate() {
        ensure_not_canceled(cancel)?;
        progress.on_event(ProgressEvent::FileStarted {
            index: idx + 1,
            total: config.inputs.len(),
        });

        let file = File::open(path)?;
        let mut reader = LossyLineReader::new(BufReader::new(file));

        loop {
            // Per-line cancellation check (matches engine.rs pattern).
            ensure_not_canceled(cancel)?;

            let n = reader.read_line(&mut line)?;
            if n == 0 {
                break;
            }

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

                // Avoid allocating Box<str> on the hot path (existing keys).
                // entry(token.into()) would allocate even for already-seen tokens.
                if let Some(v) = counts.get_mut(token) {
                    *v += 1;
                } else {
                    counts.insert(token.into(), 1);
                }

                tokens_seen += 1;
                if tokens_seen % 8_192 == 0 {
                    ensure_not_canceled(cancel)?;
                }
                if tokens_seen % 100_000 == 0 {
                    progress.on_event(ProgressEvent::TokensSeen(tokens_seen));
                }
            }
        }

        progress.on_event(ProgressEvent::FileFinished {
            index: idx + 1,
            total: config.inputs.len(),
        });
    }

    let mut result: Vec<(Box<str>, u64)> = counts.into_iter().collect();
    // Primary key: descending count.  Secondary: ascending token text for
    // fully deterministic output (stable sort not required; unstable is fine
    // here because the secondary key breaks all ties).
    result.sort_unstable_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    Ok(result)
}

// ── tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{cancel::NoCancel, config::Config, progress::NoProgress};
    use std::io::Write;
    use tempfile::NamedTempFile;

    // ── helpers ──────────────────────────────────────────────────────────────

    fn write_temp(content: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f.flush().unwrap();
        f
    }

    /// Minimal Config for frequency tests: only `inputs` matters; `output` and
    /// `output_separator` are left at defaults because token_frequency never
    /// writes a file.
    fn cfg(inputs: Vec<std::path::PathBuf>) -> Config {
        Config {
            inputs,
            ..Config::default()
        }
    }

    // ── tests ─────────────────────────────────────────────────────────────────

    #[test]
    fn empty_inputs_returns_empty_vec() {
        let result = token_frequency(&cfg(vec![]), &NoProgress, &NoCancel).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn single_token_repeated_counts_correctly() {
        let f = write_temp("apple apple apple\napple");
        let result = token_frequency(&cfg(vec![f.path().into()]), &NoProgress, &NoCancel).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0.as_ref(), "apple");
        assert_eq!(result[0].1, 4);
    }

    #[test]
    fn multiple_tokens_correct_counts() {
        // "a" appears 3 times, "b" twice, "c" once
        let f = write_temp("a b a c a b");
        let result = token_frequency(&cfg(vec![f.path().into()]), &NoProgress, &NoCancel).unwrap();
        let map: std::collections::HashMap<&str, u64> =
            result.iter().map(|(k, v)| (k.as_ref(), *v)).collect();
        assert_eq!(map["a"], 3);
        assert_eq!(map["b"], 2);
        assert_eq!(map["c"], 1);
    }

    #[test]
    fn sorted_descending_by_count_ties_alphabetical() {
        // "b"=3, "a"=3 → tie → "a" before "b" (ascending text)
        // "c"=1 → last
        let f = write_temp("b b b a a a c");
        let result = token_frequency(&cfg(vec![f.path().into()]), &NoProgress, &NoCancel).unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].0.as_ref(), "a");
        assert_eq!(result[0].1, 3);
        assert_eq!(result[1].0.as_ref(), "b");
        assert_eq!(result[1].1, 3);
        assert_eq!(result[2].0.as_ref(), "c");
        assert_eq!(result[2].1, 1);
    }

    #[test]
    fn trim_true_collapses_whitespace_variants() {
        // With default trim=true, leading/trailing whitespace is stripped from
        // each token.  TokenIter splits on whitespace so tokens already arrive
        // without surrounding spaces; trim catches any remaining edge cases.
        let f = write_temp("  hello   world  hello");
        let result = token_frequency(&cfg(vec![f.path().into()]), &NoProgress, &NoCancel).unwrap();
        let map: std::collections::HashMap<&str, u64> =
            result.iter().map(|(k, v)| (k.as_ref(), *v)).collect();
        assert_eq!(map["hello"], 2);
        assert_eq!(map["world"], 1);
    }

    #[test]
    fn drop_empty_true_skips_empty_tokens() {
        // After trim, an empty token is dropped when drop_empty=true (default).
        let f = write_temp(",,a,,b,");
        let result = token_frequency(&cfg(vec![f.path().into()]), &NoProgress, &NoCancel).unwrap();
        // Only "a" and "b" should appear; empty strings dropped.
        assert!(result.iter().all(|(k, _)| !k.is_empty()));
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn length_filter_drops_tokens_in_range() {
        // drop_length_min=1, drop_length_max=2 → drops tokens of length 1 or 2.
        let f = write_temp("a bb ccc dddd");
        let mut c = cfg(vec![f.path().into()]);
        c.drop_length_min = Some(1);
        c.drop_length_max = Some(2);
        let result = token_frequency(&c, &NoProgress, &NoCancel).unwrap();
        // "a" (len 1) and "bb" (len 2) dropped; "ccc" and "dddd" survive.
        let tokens: Vec<&str> = result.iter().map(|(k, _)| k.as_ref()).collect();
        assert!(!tokens.contains(&"a"));
        assert!(!tokens.contains(&"bb"));
        assert!(tokens.contains(&"ccc"));
        assert!(tokens.contains(&"dddd"));
    }

    #[test]
    fn multiple_files_counts_are_accumulated() {
        let f1 = write_temp("alpha beta");
        let f2 = write_temp("beta gamma alpha alpha");
        let result = token_frequency(
            &cfg(vec![f1.path().into(), f2.path().into()]),
            &NoProgress,
            &NoCancel,
        )
        .unwrap();
        let map: std::collections::HashMap<&str, u64> =
            result.iter().map(|(k, v)| (k.as_ref(), *v)).collect();
        // f1: alpha=1, beta=1 | f2: alpha=2, beta=1, gamma=1 → total: alpha=3, beta=2, gamma=1
        assert_eq!(map["alpha"], 3);
        assert_eq!(map["beta"], 2);
        assert_eq!(map["gamma"], 1);
    }

    #[test]
    fn comma_and_semicolon_are_delimiters() {
        // TokenIter splits on whitespace, comma, and semicolon.
        let f = write_temp("a,b;c,a");
        let result = token_frequency(&cfg(vec![f.path().into()]), &NoProgress, &NoCancel).unwrap();
        let map: std::collections::HashMap<&str, u64> =
            result.iter().map(|(k, v)| (k.as_ref(), *v)).collect();
        assert_eq!(map["a"], 2);
        assert_eq!(map["b"], 1);
        assert_eq!(map["c"], 1);
    }

    #[test]
    fn utf8_bom_stripped_from_first_line() {
        // LossyLineReader strips the UTF-8 BOM (\u{FEFF}) from the first line.
        let f = write_temp("\u{FEFF}hello world hello");
        let result = token_frequency(&cfg(vec![f.path().into()]), &NoProgress, &NoCancel).unwrap();
        let tokens: Vec<&str> = result.iter().map(|(k, _)| k.as_ref()).collect();
        // "hello" should appear, not "\u{FEFF}hello".
        assert!(tokens.contains(&"hello"));
        assert!(!tokens.iter().any(|t| t.starts_with('\u{FEFF}')));
        let map: std::collections::HashMap<&str, u64> =
            result.iter().map(|(k, v)| (k.as_ref(), *v)).collect();
        assert_eq!(map["hello"], 2);
    }

    #[test]
    fn lossy_utf8_input_produces_replacement_char_token() {
        // Invalid UTF-8 bytes are decoded as U+FFFD by LossyLineReader.
        let bytes: Vec<u8> = vec![b'a', b' ', 0xFF, b' ', b'a'];
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(&bytes).unwrap();
        f.flush().unwrap();

        let result = token_frequency(&cfg(vec![f.path().into()]), &NoProgress, &NoCancel).unwrap();
        let map: std::collections::HashMap<&str, u64> =
            result.iter().map(|(k, v)| (k.as_ref(), *v)).collect();
        assert_eq!(map["a"], 2);
        // The 0xFF byte becomes U+FFFD (the replacement character sequence).
        assert!(map.contains_key("\u{FFFD}"));
    }

    #[test]
    fn cancellation_returns_canceled_error() {
        use crate::cancel::{Canceled, CancellationToken};

        // Write enough content that cancellation is triggered mid-run.
        let mut content = String::new();
        for _ in 0..10_000 {
            content.push_str("token ");
        }
        let f = write_temp(&content);
        let token = CancellationToken::new();
        token.cancel();
        let err = token_frequency(&cfg(vec![f.path().into()]), &NoProgress, &token).unwrap_err();
        assert!(
            err.downcast_ref::<Canceled>().is_some(),
            "expected Canceled, got: {err}"
        );
    }

    #[test]
    fn case_sensitive_counts_distinct() {
        // Tokens are case-sensitive — "Apple" and "apple" are different keys.
        let f = write_temp("apple Apple APPLE apple");
        let result = token_frequency(&cfg(vec![f.path().into()]), &NoProgress, &NoCancel).unwrap();
        let map: std::collections::HashMap<&str, u64> =
            result.iter().map(|(k, v)| (k.as_ref(), *v)).collect();
        assert_eq!(map["apple"], 2);
        assert_eq!(map["Apple"], 1);
        assert_eq!(map["APPLE"], 1);
    }

    #[test]
    fn result_is_fully_deterministic_for_same_input() {
        let content = "z y x z y z";
        let f1 = write_temp(content);
        let f2 = write_temp(content);
        let r1 = token_frequency(&cfg(vec![f1.path().into()]), &NoProgress, &NoCancel).unwrap();
        let r2 = token_frequency(&cfg(vec![f2.path().into()]), &NoProgress, &NoCancel).unwrap();
        // Token texts and counts must match in identical order.
        let pairs1: Vec<(&str, u64)> = r1.iter().map(|(k, v)| (k.as_ref(), *v)).collect();
        let pairs2: Vec<(&str, u64)> = r2.iter().map(|(k, v)| (k.as_ref(), *v)).collect();
        assert_eq!(pairs1, pairs2);
    }
}
