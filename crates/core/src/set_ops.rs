use crate::{
    cancel::{ensure_not_canceled, CancelCheck},
    config::{Config, OutputOrdering},
    dedupe_ram::RamStore,
    progress::{ProgressEvent, ProgressSink},
    stats::Stats,
    text_line_reader::LossyLineReader,
    token_iter::TokenIter,
    writer::OutputWriter,
};
use ahash::RandomState;
use hashbrown::HashSet;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::time::Instant;

/// Which relational set operation to compute between `config.inputs` (left)
/// and the `right` slice passed to [`set_op`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetOp {
    /// Tokens present in the **left** group that do **not** appear in the right.
    Diff,
    /// Tokens present in **both** the left and right groups.
    Intersect,
    /// All unique tokens across both groups.
    ///
    /// For [`OutputOrdering::PreserveFirstSeen`] right-only tokens are appended
    /// after all left tokens, in the order they were first encountered in the
    /// right files.
    Union,
}

/// Computes a relational set operation between two groups of input files and
/// writes the deduplicated result to `config.output`.
///
/// # Operands
///
/// * **Left** — `config.inputs`. Scanned first; defines insertion order for
///   `PreserveFirstSeen` output.
/// * **Right** — the `right` slice. Scanned after the left set is fully built.
///   May be empty (e.g. `Diff` with `right = &[]` returns the full left set).
///
/// # Set semantics
///
/// | `op`              | Result                                                          |
/// |-------------------|-----------------------------------------------------------------|
/// | [`SetOp::Diff`]      | Tokens in left that do **not** appear in right.              |
/// | [`SetOp::Intersect`] | Tokens that appear in **both** left and right.               |
/// | [`SetOp::Union`]     | All unique tokens from both sides combined.                  |
///
/// # Output ordering
///
/// * [`OutputOrdering::PreserveFirstSeen`] — left tokens in first-seen order;
///   right-only tokens (Union only) appended afterwards in their own first-seen
///   order.
/// * [`OutputOrdering::Alphabetical`] — result sorted after both sets are built.
/// * [`OutputOrdering::UnorderedFast`] — no ordering guarantee.
///
/// # Errors
///
/// Calls [`Config::validate`] — returns `Err` when `config.inputs` is empty,
/// `config.output` is empty, or the output separator is empty.
///
/// # Zero impact on the core dedup engine
///
/// Runs entirely outside the [`run`] / [`run_with_control`] paths; has no
/// effect on ongoing or future dedup jobs.
///
/// [`run`]: crate::run
/// [`run_with_control`]: crate::run_with_control
pub fn set_op<P: ProgressSink, C: CancelCheck>(
    right: &[PathBuf],
    op: SetOp,
    config: &Config,
    progress: &P,
    cancel: &C,
) -> anyhow::Result<Stats> {
    config.validate()?;
    ensure_not_canceled(cancel)?;

    let left = &config.inputs;
    let total_files = left.len() + right.len();
    let started = Instant::now();
    let mut stats = Stats {
        files: total_files,
        ..Default::default()
    };

    // ── Build left store ─────────────────────────────────────────────────────
    progress.on_event(ProgressEvent::Stage("BuildingLeftSet"));

    let mut left_store = match config.ordering {
        OutputOrdering::UnorderedFast => RamStore::new_unordered(),
        OutputOrdering::PreserveFirstSeen | OutputOrdering::Alphabetical => RamStore::new_stable(),
    };
    left_store.reserve(16 * 1024);

    let (ls, ld, lf) = scan_into_store(
        left,
        0,
        total_files,
        config,
        &mut left_store,
        progress,
        cancel,
    )?;
    stats.tokens_seen += ls;
    stats.duplicates += ld;
    stats.filtered_by_length += lf;

    ensure_not_canceled(cancel)?;

    // ── Process right side + derive result tokens ─────────────────────────────
    progress.on_event(ProgressEvent::Stage("BuildingRightSet"));

    let result_tokens: Vec<Box<str>> = match op {
        SetOp::Union => {
            // Scan right into the same left_store.  Tokens already present in
            // the left set are recorded as duplicates; right-only tokens are
            // appended in their insertion order — correct for PreserveFirstSeen
            // (right-only items follow all left items) and Alphabetical (sorted
            // in the next step regardless).
            let (rs, rd, rf) = scan_into_store(
                right,
                left.len(),
                total_files,
                config,
                &mut left_store,
                progress,
                cancel,
            )?;
            stats.tokens_seen += rs;
            stats.duplicates += rd;
            stats.filtered_by_length += rf;

            ensure_not_canceled(cancel)?;
            left_store.into_tokens()
        }

        SetOp::Diff | SetOp::Intersect => {
            // Build a membership-only HashSet for the right side — O(1) lookup,
            // ordering irrelevant.
            let (membership, rs, rd, rf) =
                scan_into_membership_set(right, left.len(), total_files, config, progress, cancel)?;
            stats.tokens_seen += rs;
            stats.duplicates += rd;
            stats.filtered_by_length += rf;

            ensure_not_canceled(cancel)?;
            let left_tokens = left_store.into_tokens();

            match op {
                SetOp::Diff => left_tokens
                    .into_iter()
                    .filter(|t| !membership.contains(t.as_ref()))
                    .collect(),
                SetOp::Intersect => left_tokens
                    .into_iter()
                    .filter(|t| membership.contains(t.as_ref()))
                    .collect(),
                _ => unreachable!(),
            }
        }
    };

    // ── Apply ordering ────────────────────────────────────────────────────────
    let mut result = result_tokens;
    if matches!(config.ordering, OutputOrdering::Alphabetical) {
        ensure_not_canceled(cancel)?;
        progress.on_event(ProgressEvent::Stage("Sorting"));
        result.sort_unstable();
    }

    // ── Write output ──────────────────────────────────────────────────────────
    ensure_not_canceled(cancel)?;
    progress.on_event(ProgressEvent::Stage("WritingOutput"));
    let mut out = OutputWriter::create(&config.output, config.output_separator.clone())?;
    for token in &result {
        ensure_not_canceled(cancel)?;
        out.write_token(token)?;
    }
    out.finish()?;

    progress.on_event(ProgressEvent::Stage("Finalizing"));
    stats.unique_tokens = result.len() as u64;
    stats.elapsed = started.elapsed();
    Ok(stats)
}

// ─── private helpers ──────────────────────────────────────────────────────────

/// Scans `paths`, applies token filters from `config`, and inserts each
/// surviving token into `store`.
///
/// Progress events use absolute file indices spanning both sides:
/// file `i` in `paths` emits
/// `FileStarted { index: index_offset + i + 1, total: total_files }`.
///
/// `line` allocation is declared outside the file loop so it is reused across
/// files (same optimisation as in `engine.rs`).
///
/// Returns `(tokens_seen, duplicates_within_store, filtered_by_length)`.
fn scan_into_store<P: ProgressSink, C: CancelCheck>(
    paths: &[PathBuf],
    index_offset: usize,
    total_files: usize,
    config: &Config,
    store: &mut RamStore,
    progress: &P,
    cancel: &C,
) -> anyhow::Result<(u64, u64, u64)> {
    let mut tokens_seen: u64 = 0;
    let mut duplicates: u64 = 0;
    let mut filtered: u64 = 0;
    let mut line = String::new();

    for (i, path) in paths.iter().enumerate() {
        ensure_not_canceled(cancel)?;
        progress.on_event(ProgressEvent::FileStarted {
            index: index_offset + i + 1,
            total: total_files,
        });

        let file = File::open(path)?;
        let mut reader = LossyLineReader::new(BufReader::new(file));

        loop {
            ensure_not_canceled(cancel)?;
            let n = reader.read_line(&mut line)?;
            if n == 0 {
                break;
            }

            for raw in TokenIter::new(&line) {
                tokens_seen += 1;
                if tokens_seen % 8_192 == 0 {
                    ensure_not_canceled(cancel)?;
                }
                if tokens_seen % 100_000 == 0 {
                    progress.on_event(ProgressEvent::TokensSeen(tokens_seen));
                }

                let mut token = raw;
                if config.trim {
                    token = token.trim();
                }
                if config.drop_empty && token.is_empty() {
                    continue;
                }
                if config.should_drop_by_length(token) {
                    filtered += 1;
                    if filtered % 100_000 == 0 {
                        progress.on_event(ProgressEvent::FilteredByLength(filtered));
                    }
                    continue;
                }

                if store.insert(token) {
                    // New unique token — no separate counter here; caller
                    // derives unique_tokens from result.len() at the end.
                } else {
                    duplicates += 1;
                    if duplicates % 100_000 == 0 {
                        progress.on_event(ProgressEvent::Duplicates(duplicates));
                    }
                }
            }
        }

        progress.on_event(ProgressEvent::FileFinished {
            index: index_offset + i + 1,
            total: total_files,
        });
    }

    Ok((tokens_seen, duplicates, filtered))
}

/// Scans `paths`, applies token filters from `config`, and builds a
/// `HashSet<Box<str>, RandomState>` for O(1) membership testing.
///
/// Used as the right-side set for [`SetOp::Diff`] and [`SetOp::Intersect`]
/// where ordering is irrelevant.  The `get_mut` / `insert` hot-path pattern
/// avoids allocating a `Box<str>` for already-seen tokens.
///
/// Returns `(membership_set, tokens_seen, duplicates_within_right, filtered_by_length)`.
fn scan_into_membership_set<P: ProgressSink, C: CancelCheck>(
    paths: &[PathBuf],
    index_offset: usize,
    total_files: usize,
    config: &Config,
    progress: &P,
    cancel: &C,
) -> anyhow::Result<(HashSet<Box<str>, RandomState>, u64, u64, u64)> {
    let mut set: HashSet<Box<str>, RandomState> = HashSet::with_hasher(RandomState::new());
    set.reserve(16 * 1024);

    let mut tokens_seen: u64 = 0;
    let mut duplicates: u64 = 0;
    let mut filtered: u64 = 0;
    let mut line = String::new();

    for (i, path) in paths.iter().enumerate() {
        ensure_not_canceled(cancel)?;
        progress.on_event(ProgressEvent::FileStarted {
            index: index_offset + i + 1,
            total: total_files,
        });

        let file = File::open(path)?;
        let mut reader = LossyLineReader::new(BufReader::new(file));

        loop {
            ensure_not_canceled(cancel)?;
            let n = reader.read_line(&mut line)?;
            if n == 0 {
                break;
            }

            for raw in TokenIter::new(&line) {
                tokens_seen += 1;
                if tokens_seen % 8_192 == 0 {
                    ensure_not_canceled(cancel)?;
                }
                if tokens_seen % 100_000 == 0 {
                    progress.on_event(ProgressEvent::TokensSeen(tokens_seen));
                }

                let mut token = raw;
                if config.trim {
                    token = token.trim();
                }
                if config.drop_empty && token.is_empty() {
                    continue;
                }
                if config.should_drop_by_length(token) {
                    filtered += 1;
                    continue;
                }

                // Avoid allocating Box<str> for already-seen tokens (same
                // pattern as frequency.rs / RamStore::insert).
                if set.contains(token) {
                    duplicates += 1;
                } else {
                    set.insert(token.into());
                }
            }
        }

        progress.on_event(ProgressEvent::FileFinished {
            index: index_offset + i + 1,
            total: total_files,
        });
    }

    Ok((set, tokens_seen, duplicates, filtered))
}

// ── tests ────────────────────────────────────────────────────────────────────

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

    /// Runs `set_op` with one left file and one right file, returns a **sorted**
    /// Vec of output tokens for order-independent comparison.
    fn run_sorted(left: &str, right: &str, op: SetOp) -> Vec<String> {
        run_ordered(
            left,
            right,
            op,
            OutputOrdering::PreserveFirstSeen,
            |mut v| {
                v.sort();
                v
            },
        )
    }

    /// Runs `set_op` and applies `post` to the result tokens before returning.
    fn run_ordered<F>(
        left: &str,
        right: &str,
        op: SetOp,
        ordering: OutputOrdering,
        post: F,
    ) -> Vec<String>
    where
        F: FnOnce(Vec<String>) -> Vec<String>,
    {
        let left_f = write_temp(left);
        let right_f = write_temp(right);
        let out_dir = tempfile::tempdir().unwrap();
        let out_path = out_dir.path().join("out.txt");

        let c = Config {
            inputs: vec![left_f.path().into()],
            output: out_path.clone(),
            ordering,
            ..Config::default()
        };
        let right_paths = vec![right_f.path().to_path_buf()];
        set_op(&right_paths, op, &c, &NoProgress, &NoCancel).unwrap();

        let content = std::fs::read_to_string(&out_path).unwrap();
        if content.is_empty() {
            return vec![];
        }
        let tokens: Vec<String> = content.split('\n').map(|s| s.to_string()).collect();
        post(tokens)
    }

    // ── basic correctness ─────────────────────────────────────────────────────

    #[test]
    fn diff_removes_right_tokens() {
        // left = {a,b,c}, right = {b,c,d} → diff = {a}
        let result = run_sorted("a b c", "b c d", SetOp::Diff);
        assert_eq!(result, vec!["a"]);
    }

    #[test]
    fn intersect_keeps_common_tokens() {
        // left = {a,b}, right = {b,c} → intersect = {b}
        let result = run_sorted("a b", "b c", SetOp::Intersect);
        assert_eq!(result, vec!["b"]);
    }

    #[test]
    fn union_combines_both_sides() {
        // left = {a,b}, right = {b,c} → union = {a,b,c}
        let result = run_sorted("a b", "b c", SetOp::Union);
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    // ── empty operands ────────────────────────────────────────────────────────

    #[test]
    fn diff_empty_right_is_identity() {
        let result = run_sorted("a b c", "", SetOp::Diff);
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    #[test]
    fn intersect_empty_right_returns_empty() {
        let result = run_sorted("a b c", "", SetOp::Intersect);
        assert!(result.is_empty());
    }

    #[test]
    fn union_empty_right_returns_left() {
        let result = run_sorted("a b c", "", SetOp::Union);
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    // ── ordering guarantees ───────────────────────────────────────────────────

    #[test]
    fn diff_preserve_first_seen_order() {
        // left = c b a (insertion order), right removes b → result = c, a (left order)
        let result = run_ordered(
            "c b a",
            "b",
            SetOp::Diff,
            OutputOrdering::PreserveFirstSeen,
            |v| v,
        );
        assert_eq!(result, vec!["c", "a"]);
    }

    #[test]
    fn intersect_preserve_first_seen_order() {
        // left = c b a; intersect with {a,b} → b, a (in left insertion order)
        let result = run_ordered(
            "c b a",
            "a b",
            SetOp::Intersect,
            OutputOrdering::PreserveFirstSeen,
            |v| v,
        );
        assert_eq!(result, vec!["b", "a"]);
    }

    #[test]
    fn alphabetical_ordering_applied_to_diff() {
        // c b a minus nothing = c b a; sorted = a b c
        let result = run_ordered(
            "c b a",
            "",
            SetOp::Diff,
            OutputOrdering::Alphabetical,
            |v| v,
        );
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    #[test]
    fn union_preserve_first_seen_right_only_appended() {
        // left = a b, right = c d b
        // expected: a b (left order) then c d (right-only, right order)
        let result = run_ordered(
            "a b",
            "c d b",
            SetOp::Union,
            OutputOrdering::PreserveFirstSeen,
            |v| v,
        );
        assert_eq!(result, vec!["a", "b", "c", "d"]);
    }

    // ── identical operands ────────────────────────────────────────────────────

    #[test]
    fn diff_identical_sides_returns_empty() {
        let result = run_sorted("a b c", "a b c", SetOp::Diff);
        assert!(result.is_empty());
    }

    #[test]
    fn intersect_identical_sides_returns_full_left() {
        let result = run_sorted("a b c", "a b c", SetOp::Intersect);
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    #[test]
    fn union_identical_sides_returns_same_set() {
        let result = run_sorted("a b c", "a b c", SetOp::Union);
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    // ── stats correctness ─────────────────────────────────────────────────────

    #[test]
    fn stats_union_tokens_seen_unique_duplicates() {
        // left = "a b a"  → 3 tokens seen, 2 unique (left_dups = 1)
        // right = "b c"   → 2 tokens seen, 2 unique in right; b already in left → cross_dups = 1
        // total tokens_seen = 5, unique_tokens (output) = 3 {a,b,c}, duplicates = 2
        let left_f = write_temp("a b a");
        let right_f = write_temp("b c");
        let out_dir = tempfile::tempdir().unwrap();
        let out_path = out_dir.path().join("out.txt");

        let c = Config {
            inputs: vec![left_f.path().into()],
            output: out_path,
            ..Config::default()
        };
        let stats = set_op(
            &[right_f.path().to_path_buf()],
            SetOp::Union,
            &c,
            &NoProgress,
            &NoCancel,
        )
        .unwrap();

        assert_eq!(stats.tokens_seen, 5);
        assert_eq!(stats.unique_tokens, 3);
        assert_eq!(stats.duplicates, 2);
        assert_eq!(stats.files, 2);
    }

    #[test]
    fn stats_diff_unique_tokens_reflects_output_size() {
        // left = {a,b,c,d}, right = {b,c} → diff = {a,d} → unique_tokens = 2
        let left_f = write_temp("a b c d");
        let right_f = write_temp("b c");
        let out_dir = tempfile::tempdir().unwrap();
        let out_path = out_dir.path().join("out.txt");

        let c = Config {
            inputs: vec![left_f.path().into()],
            output: out_path,
            ..Config::default()
        };
        let stats = set_op(
            &[right_f.path().to_path_buf()],
            SetOp::Diff,
            &c,
            &NoProgress,
            &NoCancel,
        )
        .unwrap();

        assert_eq!(stats.unique_tokens, 2);
    }

    // ── multi-file operands ───────────────────────────────────────────────────

    #[test]
    fn multi_file_diff_correct() {
        // left = {a,b} ∪ {c,d} = {a,b,c,d}
        // right = {b,c} ∪ {e} = {b,c,e}
        // diff = {a,d}
        let l1 = write_temp("a b");
        let l2 = write_temp("c d");
        let r1 = write_temp("b c");
        let r2 = write_temp("e");
        let out_dir = tempfile::tempdir().unwrap();
        let out_path = out_dir.path().join("out.txt");

        let c = Config {
            inputs: vec![l1.path().into(), l2.path().into()],
            output: out_path.clone(),
            ordering: OutputOrdering::Alphabetical,
            ..Config::default()
        };
        let stats = set_op(
            &[r1.path().to_path_buf(), r2.path().to_path_buf()],
            SetOp::Diff,
            &c,
            &NoProgress,
            &NoCancel,
        )
        .unwrap();

        let content = std::fs::read_to_string(&out_path).unwrap();
        let tokens: Vec<&str> = content.split('\n').collect();
        assert_eq!(tokens, vec!["a", "d"]);
        assert_eq!(stats.unique_tokens, 2);
        assert_eq!(stats.files, 4);
    }

    #[test]
    fn multi_file_intersect_correct() {
        // left = {a,b,c}, right = {b,c,d} → intersect = {b,c}
        let l1 = write_temp("a b");
        let l2 = write_temp("c");
        let r1 = write_temp("b c d");
        let out_dir = tempfile::tempdir().unwrap();
        let out_path = out_dir.path().join("out.txt");

        let c = Config {
            inputs: vec![l1.path().into(), l2.path().into()],
            output: out_path.clone(),
            ordering: OutputOrdering::Alphabetical,
            ..Config::default()
        };
        let stats = set_op(
            &[r1.path().to_path_buf()],
            SetOp::Intersect,
            &c,
            &NoProgress,
            &NoCancel,
        )
        .unwrap();

        let content = std::fs::read_to_string(&out_path).unwrap();
        let tokens: Vec<&str> = content.split('\n').collect();
        assert_eq!(tokens, vec!["b", "c"]);
        assert_eq!(stats.unique_tokens, 2);
    }

    // ── cancellation ──────────────────────────────────────────────────────────

    #[test]
    fn cancellation_during_left_scan_returns_canceled() {
        use crate::cancel::{Canceled, CancellationToken};

        let mut content = String::new();
        for _ in 0..10_000 {
            content.push_str("token ");
        }
        let left_f = write_temp(&content);
        let out_dir = tempfile::tempdir().unwrap();
        let out_path = out_dir.path().join("out.txt");

        let c = Config {
            inputs: vec![left_f.path().into()],
            output: out_path,
            ..Config::default()
        };
        let token = CancellationToken::new();
        token.cancel();
        let err = set_op(&[], SetOp::Diff, &c, &NoProgress, &token).unwrap_err();
        assert!(
            err.downcast_ref::<Canceled>().is_some(),
            "expected Canceled, got: {err}"
        );
    }

    // ── filter settings ───────────────────────────────────────────────────────

    #[test]
    fn length_filter_applied_to_both_sides() {
        // drop_length_min=1, drop_length_max=1 → drop single-char tokens
        // left = {a, bb, ccc}, right = {bb, d, eee}
        // After filter: left = {bb, ccc}, right = {bb, eee}
        // Diff = {ccc}
        let left_f = write_temp("a bb ccc");
        let right_f = write_temp("bb d eee");
        let out_dir = tempfile::tempdir().unwrap();
        let out_path = out_dir.path().join("out.txt");

        let c = Config {
            inputs: vec![left_f.path().into()],
            output: out_path.clone(),
            drop_length_min: Some(1),
            drop_length_max: Some(1),
            ..Config::default()
        };
        set_op(
            &[right_f.path().to_path_buf()],
            SetOp::Diff,
            &c,
            &NoProgress,
            &NoCancel,
        )
        .unwrap();

        let content = std::fs::read_to_string(&out_path).unwrap();
        assert_eq!(content, "ccc");
    }

    // ── within-left dedup (left has its own duplicates) ───────────────────────

    #[test]
    fn within_left_duplicates_are_deduplicated_before_op() {
        // left = "a a b b" → unique left = {a, b}
        // right = "b" → diff = {a}
        let result = run_sorted("a a b b", "b", SetOp::Diff);
        assert_eq!(result, vec!["a"]);
    }

    #[test]
    fn within_right_duplicates_do_not_affect_membership() {
        // right = "b b b" → membership = {b}; diff of {a,b,c} = {a,c}
        let result = run_sorted("a b c", "b b b", SetOp::Diff);
        assert_eq!(result, vec!["a", "c"]);
    }
}
