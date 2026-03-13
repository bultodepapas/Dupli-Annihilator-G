use crate::{
    cancel::{ensure_not_canceled, CancelCheck},
    config::{Config, OutputOrdering},
    dedupe_ram::RamStore,
    progress::{ProgressEvent, ProgressSink},
    text_line_reader::LossyLineReader,
    token_iter::TokenIter,
    writer::OutputWriter,
};
use ahash::RandomState;
use hashbrown::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::time::Instant;

/// Statistics returned by [`fuzzy_dedup_inputs`].
#[derive(Debug, Clone)]
pub struct FuzzyStats {
    /// Total tokens observed across all input files (before exact dedup).
    pub tokens_seen: u64,
    /// Distinct tokens after exact dedup — the number fed into the fuzzy pass.
    pub exact_unique: u64,
    /// Clusters produced by the fuzzy pass — the number of tokens written to
    /// the output file.
    pub clusters_found: u64,
    /// Wall-clock time for the entire operation (both phases).
    pub elapsed: std::time::Duration,
}

// ── Union-Find (path halving + union by rank) ─────────────────────────────────

struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<u8>,
}

impl UnionFind {
    fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
            rank: vec![0; n],
        }
    }

    /// Path-halving `find` — amortised O(α) per call; avoids recursion.
    fn find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            // Point x to its grandparent, then follow.
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    /// Union by rank — keeps the tree shallow.
    fn union(&mut self, x: usize, y: usize) {
        let rx = self.find(x);
        let ry = self.find(y);
        if rx == ry {
            return;
        }
        match self.rank[rx].cmp(&self.rank[ry]) {
            std::cmp::Ordering::Less => self.parent[rx] = ry,
            std::cmp::Ordering::Greater => self.parent[ry] = rx,
            std::cmp::Ordering::Equal => {
                self.parent[ry] = rx;
                self.rank[rx] += 1;
            }
        }
    }
}

// ── Bounded Levenshtein distance ──────────────────────────────────────────────

/// Returns the Levenshtein edit distance between `a` and `b`, capped at
/// `max + 1` when the true distance exceeds `max`.
///
/// Operates on Unicode scalar values (chars) so multibyte characters count as
/// one edit — consistent with the tokenisation model used by the dedup engine.
///
/// Uses a single rolling row (O(min(|a|, |b|)) space) and applies two pruning
/// strategies to exit early:
///
/// 1. **Length-difference early exit** — if `|la − lb| > max` the distance is
///    already `> max` without examining any cells.
/// 2. **Row-minimum pruning** — once the minimum value in the current DP row
///    exceeds `max`, no subsequent row can produce a smaller result.
fn levenshtein_bounded(a: &str, b: &str, max: usize) -> usize {
    // Collect into char vecs; swap so the shorter string is `a` (smaller row).
    let mut a_chars: Vec<char> = a.chars().collect();
    let mut b_chars: Vec<char> = b.chars().collect();
    if a_chars.len() > b_chars.len() {
        std::mem::swap(&mut a_chars, &mut b_chars);
    }
    let la = a_chars.len();
    let lb = b_chars.len();

    // After the swap la ≤ lb, so lb − la never underflows.
    if lb - la > max {
        return max + 1;
    }
    if la == 0 {
        return lb;
    }

    // prev[j] = edit distance between a[0..i] and b[0..j] for the previous i.
    let mut prev: Vec<usize> = (0..=lb).collect();
    let mut curr: Vec<usize> = vec![0; lb + 1];

    for (i, &ca) in a_chars.iter().enumerate() {
        curr[0] = i + 1;
        let mut row_min = curr[0];

        for (j, &cb) in b_chars.iter().enumerate() {
            let sub = usize::from(ca != cb);
            curr[j + 1] = (prev[j + 1] + 1).min(curr[j] + 1).min(prev[j] + sub);
            if curr[j + 1] < row_min {
                row_min = curr[j + 1];
            }
        }

        if row_min > max {
            return max + 1;
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[lb]
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Groups `tokens` into near-duplicate clusters and returns one representative
/// (the **lexicographically smallest** member) per cluster.
///
/// Two tokens end up in the same cluster when their Levenshtein edit distance
/// is ≤ `max_edit`, directly *or* transitively (Union-Find semantics).
///
/// # Output order
///
/// The returned `Vec` is in **unspecified order**; callers that need a specific
/// order should sort afterwards.  [`fuzzy_dedup_inputs`] handles ordering
/// according to `Config::ordering`.
///
/// # Complexity
///
/// O(u²) pairwise comparisons where `u = tokens.len()`.  Practical upper bound
/// is ~100 000 unique tokens; above that the quadratic cost dominates.
///
/// # Cancellation
///
/// `cancel` is checked once every 8 192 pairwise comparisons.
pub fn fuzzy_cluster<C: CancelCheck>(
    tokens: &[Box<str>],
    max_edit: usize,
    cancel: &C,
) -> anyhow::Result<Vec<Box<str>>> {
    let n = tokens.len();
    if n == 0 {
        return Ok(Vec::new());
    }

    let mut uf = UnionFind::new(n);
    let mut comparisons: u64 = 0;

    for i in 0..n {
        for j in (i + 1)..n {
            comparisons += 1;
            if comparisons % 8_192 == 0 {
                ensure_not_canceled(cancel)?;
            }
            if levenshtein_bounded(&tokens[i], &tokens[j], max_edit) <= max_edit {
                uf.union(i, j);
            }
        }
    }

    // For each cluster root collect the index of the lex-smallest token.
    let mut cluster_min: HashMap<usize, usize, RandomState> =
        HashMap::with_capacity_and_hasher(n / 2 + 1, RandomState::new());

    for idx in 0..n {
        let root = uf.find(idx);
        cluster_min
            .entry(root)
            .and_modify(|best| {
                if tokens[idx] < tokens[*best] {
                    *best = idx;
                }
            })
            .or_insert(idx);
    }

    Ok(cluster_min
        .into_values()
        .map(|idx| tokens[idx].clone())
        .collect())
}

/// Full pipeline: reads `config.inputs`, performs exact deduplication, runs
/// the fuzzy clustering pass, applies output ordering, and writes the result
/// to `config.output`.
///
/// `max_edit` is the Levenshtein distance threshold — typically 1 or 2.
///
/// # Two-phase design
///
/// * **Phase 1 (exact dedup)** — identical tokenisation to the dedup engine;
///   drastically reduces the token count before the expensive fuzzy pass.
/// * **Phase 2 (fuzzy cluster)** — operates on the small exact-unique set,
///   not the full corpus.
///
/// # Zero impact on the core dedup engine
///
/// Runs entirely outside the [`run`] / [`run_with_control`] paths.
///
/// [`run`]: crate::run
/// [`run_with_control`]: crate::run_with_control
pub fn fuzzy_dedup_inputs<P: ProgressSink, C: CancelCheck>(
    config: &Config,
    max_edit: usize,
    progress: &P,
    cancel: &C,
) -> anyhow::Result<FuzzyStats> {
    anyhow::ensure!(
        max_edit >= 1 && max_edit <= 10,
        "max_edit must be between 1 and 10, got {max_edit}"
    );
    config.validate()?;

    let started = Instant::now();
    let total_files = config.inputs.len();

    // ── Phase 1: exact dedup ──────────────────────────────────────────────────

    progress.on_event(ProgressEvent::Stage("FuzzyExactDedup"));

    // Use a stable store (preserves insertion order) for PreserveFirstSeen and
    // Alphabetical; unordered for UnorderedFast.
    let mut store = match config.ordering {
        OutputOrdering::UnorderedFast => RamStore::new_unordered(),
        OutputOrdering::PreserveFirstSeen | OutputOrdering::Alphabetical => RamStore::new_stable(),
    };
    store.reserve(16 * 1024);

    let mut tokens_seen: u64 = 0;
    // Buffer reused across all files — LossyLineReader::read_line clears it.
    let mut line = String::new();

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
            let n = reader.read_line(&mut line)?;
            if n == 0 {
                break;
            }
            for raw in TokenIter::new(&line) {
                let token = if config.trim { raw.trim() } else { raw };
                if config.drop_empty && token.is_empty() {
                    continue;
                }
                if config.should_drop_by_length(token) {
                    continue;
                }
                store.insert(token);
                tokens_seen += 1;
                if tokens_seen % 8_192 == 0 {
                    ensure_not_canceled(cancel)?;
                }
            }
        }

        progress.on_event(ProgressEvent::FileFinished {
            index: idx + 1,
            total: total_files,
        });
    }

    // Materialise as Vec; for a Stable store this preserves insertion order.
    let exact_tokens: Vec<Box<str>> = store.into_tokens();
    let exact_unique = exact_tokens.len() as u64;

    // ── Phase 2: fuzzy clustering ─────────────────────────────────────────────

    progress.on_event(ProgressEvent::Stage("FuzzyClustering"));

    let mut representatives = fuzzy_cluster(&exact_tokens, max_edit, cancel)?;
    let clusters_found = representatives.len() as u64;

    // ── Apply output ordering ─────────────────────────────────────────────────

    match config.ordering {
        OutputOrdering::Alphabetical => {
            representatives.sort_unstable();
        }
        OutputOrdering::PreserveFirstSeen => {
            // Build a position map: token_str → original insertion index.
            // Every representative is guaranteed to be in exact_tokens.
            let mut pos: HashMap<&str, usize, RandomState> =
                HashMap::with_capacity_and_hasher(exact_tokens.len(), RandomState::new());
            for (i, t) in exact_tokens.iter().enumerate() {
                pos.insert(t.as_ref(), i);
            }
            representatives
                .sort_unstable_by_key(|t| pos.get(t.as_ref()).copied().unwrap_or(usize::MAX));
        }
        OutputOrdering::UnorderedFast => {
            // No ordering guarantee required.
        }
    }

    // ── Write output ──────────────────────────────────────────────────────────

    let mut writer = OutputWriter::create(&config.output, config.output_separator.clone())?;
    for token in &representatives {
        writer.write_token(token)?;
    }
    writer.finish()?;

    Ok(FuzzyStats {
        tokens_seen,
        exact_unique,
        clusters_found,
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

    fn to_boxed(v: &[&str]) -> Vec<Box<str>> {
        v.iter().map(|s| Box::from(*s)).collect()
    }

    fn sorted_strings(v: Vec<Box<str>>) -> Vec<String> {
        let mut s: Vec<String> = v.into_iter().map(String::from).collect();
        s.sort();
        s
    }

    fn read_output(path: &std::path::Path) -> Vec<String> {
        let content = std::fs::read_to_string(path).unwrap();
        if content.is_empty() {
            vec![]
        } else {
            content.split('\n').map(str::to_string).collect()
        }
    }

    fn cfg_with_output(inputs: Vec<std::path::PathBuf>, output: std::path::PathBuf) -> Config {
        Config {
            inputs,
            output,
            trim: true,
            drop_empty: true,
            ..Config::default()
        }
    }

    // ── levenshtein_bounded ───────────────────────────────────────────────────

    #[test]
    fn lev_identical_strings_is_zero() {
        assert_eq!(levenshtein_bounded("hello", "hello", 3), 0);
    }

    #[test]
    fn lev_empty_a_is_length_of_b() {
        assert_eq!(levenshtein_bounded("", "abc", 5), 3);
    }

    #[test]
    fn lev_single_substitution() {
        assert_eq!(levenshtein_bounded("colour", "color", 2), 1);
    }

    #[test]
    fn lev_capped_when_exceeds_max() {
        assert_eq!(levenshtein_bounded("abcdef", "xyz", 2), 3); // true dist=6, returns max+1=3
    }

    #[test]
    fn lev_length_diff_early_exit() {
        // Length diff = 4 > max = 2 → early exit without DP.
        assert_eq!(levenshtein_bounded("a", "aaaaa", 2), 3);
    }

    #[test]
    fn lev_unicode_chars_count_as_one() {
        // "café" vs "cafe": one substitution (é→e), not multiple bytes.
        assert_eq!(levenshtein_bounded("café", "cafe", 2), 1);
    }

    // ── fuzzy_cluster (pure) ──────────────────────────────────────────────────

    #[test]
    fn empty_tokens_returns_empty() {
        let result = fuzzy_cluster(&[], 1, &NoCancel).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn single_token_returns_itself() {
        let tokens = to_boxed(&["hello"]);
        let result = fuzzy_cluster(&tokens, 1, &NoCancel).unwrap();
        assert_eq!(sorted_strings(result), vec!["hello"]);
    }

    #[test]
    fn no_near_duplicates_all_separate() {
        let tokens = to_boxed(&["apple", "mango", "cherry"]);
        let result = fuzzy_cluster(&tokens, 1, &NoCancel).unwrap();
        // All pairwise distances > 1 → each is its own cluster.
        assert_eq!(sorted_strings(result), vec!["apple", "cherry", "mango"]);
    }

    #[test]
    fn one_substitution_clusters_pair() {
        // "colour" / "color": 1 edit → clustered; lex-min is "color".
        let tokens = to_boxed(&["colour", "color"]);
        let result = fuzzy_cluster(&tokens, 1, &NoCancel).unwrap();
        assert_eq!(sorted_strings(result), vec!["color"]);
    }

    #[test]
    fn transitive_clustering_three_tokens() {
        // "abc"↔"abd" (1 edit), "abd"↔"abe" (1 edit) → all three in one cluster.
        // lex-min is "abc".
        let tokens = to_boxed(&["abc", "abd", "abe"]);
        let result = fuzzy_cluster(&tokens, 1, &NoCancel).unwrap();
        assert_eq!(sorted_strings(result), vec!["abc"]);
    }

    #[test]
    fn lex_smallest_is_representative() {
        // "teh" and "the" differ by 2 edits (swap) — within max_edit=2.
        // lex-min: "teh" < "the".
        let tokens = to_boxed(&["the", "teh"]);
        let result = fuzzy_cluster(&tokens, 2, &NoCancel).unwrap();
        assert_eq!(sorted_strings(result), vec!["teh"]);
    }

    #[test]
    fn max_edit_2_clusters_more_aggressively() {
        // "hello" / "helo" (1 edit) / "heo" (2 edits from "hello") → one cluster.
        let tokens = to_boxed(&["hello", "helo", "heo"]);
        let result = fuzzy_cluster(&tokens, 2, &NoCancel).unwrap();
        assert_eq!(sorted_strings(result).len(), 1);
    }

    #[test]
    fn different_clusters_preserved() {
        // "cat"/"hat" (1 edit) cluster together; "dog" is alone.
        let tokens = to_boxed(&["cat", "hat", "dog"]);
        let result = fuzzy_cluster(&tokens, 1, &NoCancel).unwrap();
        let sorted = sorted_strings(result);
        assert_eq!(sorted.len(), 2);
        assert!(sorted.contains(&"cat".to_string()));
        assert!(sorted.contains(&"dog".to_string()));
    }

    #[test]
    fn cancellation_returns_canceled_error() {
        use crate::cancel::CancellationToken;
        let tok = CancellationToken::new();
        tok.cancel();
        // Build a slice large enough to trigger the cancellation check.
        let tokens: Vec<Box<str>> = (0..200u32)
            .map(|i| format!("token{i:04}").into_boxed_str())
            .collect();
        let err = fuzzy_cluster(&tokens, 1, &tok).unwrap_err();
        assert!(crate::cancel::is_canceled_error(&err));
    }

    // ── fuzzy_dedup_inputs (full pipeline) ───────────────────────────────────

    #[test]
    fn pipeline_clusters_typos_and_writes_output() {
        let input = write_temp("colour\ncolor\napple\n");
        let out = NamedTempFile::new().unwrap();
        let cfg = cfg_with_output(vec![input.path().into()], out.path().to_path_buf());
        let stats = fuzzy_dedup_inputs(&cfg, 1, &NoProgress, &NoCancel).unwrap();
        assert_eq!(stats.tokens_seen, 3);
        assert_eq!(stats.exact_unique, 3); // all three distinct
        assert_eq!(stats.clusters_found, 2); // "colour"/"color" → 1; "apple" → 1
        let lines = read_output(out.path());
        assert_eq!(lines.len(), 2);
        assert!(lines.contains(&"color".to_string()));
        assert!(lines.contains(&"apple".to_string()));
    }

    #[test]
    fn pipeline_exact_duplicates_deduped_before_fuzzy() {
        // "apple" appears 3 times; exact dedup reduces to 1 before fuzzy pass.
        let input = write_temp("apple\napple\napple\n");
        let out = NamedTempFile::new().unwrap();
        let cfg = cfg_with_output(vec![input.path().into()], out.path().to_path_buf());
        let stats = fuzzy_dedup_inputs(&cfg, 1, &NoProgress, &NoCancel).unwrap();
        assert_eq!(stats.tokens_seen, 3);
        assert_eq!(stats.exact_unique, 1);
        assert_eq!(stats.clusters_found, 1);
    }

    #[test]
    fn pipeline_empty_input_writes_empty_output() {
        let input = write_temp("");
        let out = NamedTempFile::new().unwrap();
        let cfg = cfg_with_output(vec![input.path().into()], out.path().to_path_buf());
        let stats = fuzzy_dedup_inputs(&cfg, 1, &NoProgress, &NoCancel).unwrap();
        assert_eq!(stats.tokens_seen, 0);
        assert_eq!(stats.exact_unique, 0);
        assert_eq!(stats.clusters_found, 0);
        assert!(read_output(out.path()).is_empty());
    }

    #[test]
    fn pipeline_alphabetical_ordering() {
        let input = write_temp("banana\nbanana\nbarana\napple\n");
        let out = NamedTempFile::new().unwrap();
        let mut cfg = cfg_with_output(vec![input.path().into()], out.path().to_path_buf());
        cfg.ordering = OutputOrdering::Alphabetical;
        fuzzy_dedup_inputs(&cfg, 1, &NoProgress, &NoCancel).unwrap();
        let lines = read_output(out.path());
        // "banana"/"barana" → cluster rep "banana"; "apple" alone.
        // Alphabetical: ["apple", "banana"]
        assert_eq!(lines, vec!["apple", "banana"]);
    }

    #[test]
    fn pipeline_preserve_first_seen_ordering() {
        // Insertion order: "zzz" first, then "apple"/"appl" cluster.
        let input = write_temp("zzz\napple\nappl\n");
        let out = NamedTempFile::new().unwrap();
        let mut cfg = cfg_with_output(vec![input.path().into()], out.path().to_path_buf());
        cfg.ordering = OutputOrdering::PreserveFirstSeen;
        fuzzy_dedup_inputs(&cfg, 1, &NoProgress, &NoCancel).unwrap();
        let lines = read_output(out.path());
        // "zzz" seen first → comes first; rep for apple cluster is "appl" (lex-min).
        assert_eq!(lines, vec!["zzz", "appl"]);
    }

    #[test]
    fn pipeline_invalid_max_edit_returns_error() {
        let input = write_temp("hello\n");
        let out = NamedTempFile::new().unwrap();
        let cfg = cfg_with_output(vec![input.path().into()], out.path().to_path_buf());
        assert!(fuzzy_dedup_inputs(&cfg, 0, &NoProgress, &NoCancel).is_err());
        assert!(fuzzy_dedup_inputs(&cfg, 11, &NoProgress, &NoCancel).is_err());
    }
}
