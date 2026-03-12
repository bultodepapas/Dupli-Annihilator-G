# Proposed Functions — Dupli-Annihilator-G

> **Author:** Senior Rust Developer / Data Science perspective
> **Date:** 2026-03-12
> **Codebase version:** v2.6.3
> **Scope:** Additions to `crates/core` — architecturally consistent, production-quality proposals.

---

## Context

The engine already covers exact deduplication in RAM and DISK modes with cancellation, progress, and rich-file extraction. The five functions below target the gaps that matter most for real data-science workflows: **frequency analysis**, **set operations across files**, **fuzzy/near-duplicate detection**, **n-gram extraction**, and **streaming Bloom-filter dedup**. Each proposal includes the full function signature, the target module, key design choices, and concrete use cases.

---

## 1. `token_frequency` — Token Frequency Map (RAM)

**Target module:** `crates/core/src/frequency.rs` (new)
**Exposed from:** `crates/core/src/lib.rs`

### Motivation

Deduplication tells you *what is unique*; frequency tells you *what matters*. A frequency map is the foundation of TF-IDF, vocabulary pruning, corpus statistics, and anomaly detection in log files. The existing `RamStore` throws away count information — this function keeps it.

### Signature

```rust
use std::path::Path;
use ahash::AHashMap;
use anyhow::Result;

/// Reads all `inputs`, tokenises with the same rules as the dedup engine,
/// and returns a frequency map: token → occurrence count.
///
/// Tokens are normalised according to `trim` and length-filter settings from
/// `config`.  The map is sorted descending by count before return so callers
/// can take the top-N directly.
pub fn token_frequency(
    inputs: &[&Path],
    config: &Config,
    cancel: &impl CancelCheck,
    sink: &impl ProgressSink,
) -> Result<Vec<(Box<str>, u64)>>
```

### Core Implementation Sketch

```rust
// crates/core/src/frequency.rs

pub fn token_frequency(
    inputs: &[&Path],
    config: &Config,
    cancel: &impl CancelCheck,
    sink: &impl ProgressSink,
) -> Result<Vec<(Box<str>, u64)>> {
    let mut counts: AHashMap<Box<str>, u64> = AHashMap::new();
    let mut seen: u64 = 0;

    for path in inputs {
        sink.on_event(ProgressEvent::FileStarted { path: path.to_path_buf() });
        let file = BufReader::new(File::open(path)?);
        let mut reader = LossyLineReader::new(file);
        let mut line = String::new();

        while reader.read_line(&mut line)? > 0 {
            for token in TokenIter::new(&line) {
                let t = if config.trim { token.trim() } else { token };
                if t.is_empty() && config.drop_empty { line.clear(); continue; }
                // length filters
                if let Some(min) = config.drop_length_min {
                    if t.chars().count() < min { line.clear(); continue; }
                }
                *counts.entry(t.into()).or_insert(0) += 1;
                seen += 1;
                if seen % 8_192 == 0 { ensure_not_canceled(cancel)?; }
            }
            line.clear();
        }
        sink.on_event(ProgressEvent::FileFinished { path: path.to_path_buf() });
    }

    let mut vec: Vec<(Box<str>, u64)> = counts.into_iter().collect();
    vec.sort_unstable_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    Ok(vec)
}
```

### Why This Design

| Choice | Reason |
|--------|--------|
| `AHashMap` (not `BTreeMap`) | O(1) insert; sort happens once at the end — same perf philosophy as the dedup engine |
| Sorted descending by count | Callers doing "top 1000 words" take a slice with no extra work |
| Reuses `LossyLineReader` + `TokenIter` | Identical tokenisation to `run()` — counts are consistent with dedup output |
| `CancelCheck` + `ProgressSink` | Works inside the existing `JobManager` pipeline; fits `BridgeSink` transparently |

### Use Cases

- **Vocabulary building** for NLP models — prune tokens below threshold count
- **Log analysis** — find the 20 most common error messages across 50 log files
- **Corpus statistics** — type-token ratio, Zipf distribution checks
- **Input validation** — detect unexpectedly high-frequency tokens before dedup

---

## 2. `set_diff` / `set_intersect` / `set_union` — Multi-File Set Operations

**Target module:** `crates/core/src/set_ops.rs` (new)
**Exposed from:** `crates/core/src/lib.rs`

### Motivation

The current engine merges all inputs into one deduplicated set. Real data-science pipelines need **relational set operations**: "what words appear in the blacklist but not my corpus?", "what tokens are common to both datasets?". These are a one-function generalisation of the existing RAM dedup path.

### Signatures

```rust
pub enum SetOp { Diff, Intersect, Union }

/// Computes a set operation between two input groups.
///
/// - `Diff`       → tokens in `left` that are NOT in `right`
/// - `Intersect`  → tokens present in BOTH `left` and `right`
/// - `Union`      → all unique tokens across both groups (equivalent to merging all inputs)
///
/// Output ordering respects `config.ordering`.
pub fn set_op(
    left: &[&Path],
    right: &[&Path],
    op: SetOp,
    config: &Config,
    cancel: &impl CancelCheck,
    sink: &impl ProgressSink,
) -> Result<Vec<Box<str>>>
```

### Core Implementation Sketch

```rust
// crates/core/src/set_ops.rs

pub fn set_op(
    left: &[&Path],
    right: &[&Path],
    op: SetOp,
    config: &Config,
    cancel: &impl CancelCheck,
    sink: &impl ProgressSink,
) -> Result<Vec<Box<str>>> {
    // Build sets for both sides using the same RAM pipeline
    let left_set  = collect_token_set(left,  config, cancel, sink)?;
    let right_set = collect_token_set(right, config, cancel, sink)?;

    let result: Vec<Box<str>> = match op {
        SetOp::Diff      => left_set.iter()
                               .filter(|t| !right_set.contains(t.as_ref()))
                               .cloned().collect(),
        SetOp::Intersect => left_set.iter()
                               .filter(|t| right_set.contains(t.as_ref()))
                               .cloned().collect(),
        SetOp::Union     => {
            let mut u = left_set;
            for t in right_set { u.insert(t); }
            u.into_iter().collect()
        }
    };

    // Reuse existing ordering logic
    apply_ordering(result, config.ordering)
}

fn collect_token_set(
    paths: &[&Path],
    config: &Config,
    cancel: &impl CancelCheck,
    sink: &impl ProgressSink,
) -> Result<HashSet<Box<str>, AHasher>> { /* same loop as run_ram */ }
```

### Why This Design

| Choice | Reason |
|--------|--------|
| Two-set RAM approach | Both sets fit in memory for typical use; DISK variant is a natural follow-up using bucket intersection |
| `apply_ordering` reuse | Keeps output-ordering logic DRY — same alphabetical/preserve/unordered paths as the main engine |
| Single `SetOp` enum | One Tauri command, one CLI flag (`--set-op diff/intersect/union`) — minimal API surface |

### Use Cases

- **Blocklist enforcement** — `Diff(corpus, blocklist)` removes banned words
- **Cross-dataset analysis** — `Intersect(dataset_A, dataset_B)` finds shared vocabulary
- **Incremental update** — `Diff(new_data, already_processed)` emits only genuinely new tokens
- **Dedup merge** — `Union` is the existing `run()` but now composable with the other ops

---

## 3. `fuzzy_cluster` — Near-Duplicate Token Clustering

**Target module:** `crates/core/src/fuzzy.rs` (new)
**Exposed from:** `crates/core/src/lib.rs`

### Motivation

Exact dedup misses typos, OCR errors, and morphological variants (`colour`/`color`, `analyse`/`analyze`). A fuzzy dedup pass clusters near-duplicates and keeps one canonical representative per cluster — critical for cleaning NLP training data, product catalogues, and OCR output.

### Signature

```rust
/// Groups tokens into clusters where every member is within `max_edit_distance`
/// of the cluster's canonical form (the lexicographically smallest member).
///
/// Returns one representative per cluster, maintaining `config.ordering`.
/// Complexity: O(u²) in unique-token count `u`; suitable for u ≤ ~100k tokens.
pub fn fuzzy_cluster(
    tokens: &[Box<str>],           // already-deduped exact set
    max_edit_distance: usize,      // typically 1 or 2
    config: &Config,
    cancel: &impl CancelCheck,
) -> Result<Vec<Box<str>>>
```

### Core Implementation Sketch

```rust
// crates/core/src/fuzzy.rs

pub fn fuzzy_cluster(
    tokens: &[Box<str>],
    max_edit: usize,
    config: &Config,
    cancel: &impl CancelCheck,
) -> Result<Vec<Box<str>>> {
    // Union-Find for cluster membership
    let mut parent: Vec<usize> = (0..tokens.len()).collect();

    for i in 0..tokens.len() {
        for j in (i + 1)..tokens.len() {
            if j % 8_192 == 0 { ensure_not_canceled(cancel)?; }
            if levenshtein(&tokens[i], &tokens[j]) <= max_edit {
                union(&mut parent, i, j);
            }
        }
    }

    // Keep lexicographically smallest representative per cluster
    let mut clusters: AHashMap<usize, &Box<str>> = AHashMap::new();
    for (idx, tok) in tokens.iter().enumerate() {
        let root = find(&parent, idx);
        clusters
            .entry(root)
            .and_modify(|cur| { if tok < cur { *cur = tok } })
            .or_insert(tok);
    }

    let mut result: Vec<Box<str>> = clusters.into_values().cloned().collect();
    apply_ordering(result, config.ordering)
}

/// Iterative Levenshtein with early exit at `max_edit + 1`.
fn levenshtein(a: &str, b: &str) -> usize { /* standard DP, O(|a||b|) */ }
```

### Why This Design

| Choice | Reason |
|--------|--------|
| Operates on already-deduped tokens | Exact dedup first (fast, O(n)) reduces `u` drastically; fuzzy pass runs on the small exact-unique set |
| Union-Find (path compression) | O(α) amortised per operation; correct clustering even with transitive chains (`abc→abd→abe`) |
| Levenshtein with early exit | Strings differing in length by more than `max_edit` skip the DP matrix entirely |
| Lexicographically smallest representative | Deterministic, reproducible output — same input always produces same canonical form |

### Use Cases

- **OCR cleanup** — cluster `"teh"/"the"`, `"recieve"/"receive"`
- **Product catalogue normalisation** — `"t-shirt"/"tshirt"/"T shirt"` → one entry
- **Training data quality** — find morphological duplicates before tokenising for LLMs
- **Log deduplication** — cluster slightly varying error messages under one representative

---

## 4. `ngram_extract` — N-gram Extraction Pipeline

**Target module:** `crates/core/src/ngram.rs` (new)
**Exposed from:** `crates/core/src/lib.rs`

### Motivation

Single-token dedup discards sequence information. N-grams (bigrams, trigrams, …) capture **co-occurrence patterns** essential for phrase detection, language model training, and collocation analysis. This function slots into the existing pipeline: it tokenises inputs, slides an n-gram window, deduplicates the resulting compound tokens, and writes them to the output file.

### Signature

```rust
/// Extracts all unique n-grams of size `n` from `inputs`, writing them to
/// `config.output` using the same separator rules as the main engine.
///
/// An n-gram is represented as tokens joined by a single space, e.g.
/// `"machine learning"` for n=2.  Ordering respects `config.ordering`.
pub fn ngram_extract(
    n: usize,                      // 2 = bigrams, 3 = trigrams, etc.
    config: &Config,
    cancel: &impl CancelCheck,
    sink: &impl ProgressSink,
) -> Result<Stats>
```

### Core Implementation Sketch

```rust
// crates/core/src/ngram.rs

pub fn ngram_extract(
    n: usize,
    config: &Config,
    cancel: &impl CancelCheck,
    sink: &impl ProgressSink,
) -> Result<Stats> {
    assert!(n >= 2, "n must be ≥ 2");

    let mut store = RamStore::new(config.ordering);
    let mut window: VecDeque<String> = VecDeque::with_capacity(n);
    let mut tokens_seen: u64 = 0;

    for path in &config.inputs {
        let file = BufReader::new(File::open(path)?);
        let mut reader = LossyLineReader::new(file);
        let mut line = String::new();

        while reader.read_line(&mut line)? > 0 {
            for token in TokenIter::new(&line) {
                let t = token.to_string();
                window.push_back(t);
                if window.len() == n {
                    // Emit n-gram as space-joined compound token
                    let gram: String = window.iter()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(" ");
                    store.insert(gram.into_boxed_str());
                    window.pop_front();
                }
                tokens_seen += 1;
                if tokens_seen % 8_192 == 0 { ensure_not_canceled(cancel)?; }
            }
            // Reset window at line boundaries (don't span lines)
            window.clear();
            line.clear();
        }
    }

    let unique_tokens = store.len() as u64;
    let unique = store.into_tokens();
    let mut writer = OutputWriter::new(&config.output, &config.output_separator)?;
    for gram in &unique { writer.write_token(gram)?; }

    Ok(Stats { tokens_seen, unique_tokens, duplicates: tokens_seen - unique_tokens, ..Default::default() })
}
```

### Why This Design

| Choice | Reason |
|--------|--------|
| `VecDeque` sliding window | O(1) push/pop at both ends; minimal allocation per n-gram |
| Window reset at line boundaries | Prevents cross-sentence bigrams like `"period The"` — standard NLP practice |
| Space-joined compound token | Output is human-readable and directly importable into pandas/numpy without extra parsing |
| Reuses `RamStore` + `OutputWriter` | Deduplication and output formatting are free — zero new code paths for those concerns |

### Use Cases

- **Phrase extraction** for keyword research, SEO analysis
- **Language model n-gram corpus** — deduplicated bigram/trigram tables
- **Co-occurrence matrix building** — feed bigrams into downstream sparse matrix tools
- **Text fingerprinting** — shingling (n-grams as document features for similarity search)

---

## 5. `bloom_stream_dedupe` — Streaming Bloom-Filter Deduplication

**Target module:** `crates/core/src/bloom.rs` (new)
**Exposed from:** `crates/core/src/lib.rs`

### Motivation

Both RAM mode (exact, all-in-memory) and DISK mode (exact, two-pass) require either abundant RAM or two filesystem passes. **Bloom-filter streaming dedup** is a single-pass, constant-memory algorithm that accepts a configurable false-positive rate. For data-science pipelines processing **hundreds of GB of tokens** where a small number of false positives (duplicate leakage ≤ 0.1%) is acceptable, this is the right trade-off.

No external crate is needed — a minimal Bloom filter over AHash is ~50 lines.

### Signature

```rust
/// Single-pass streaming deduplication using a Bloom filter.
///
/// Outputs tokens that are *probably new* (false positives possible at rate
/// `false_positive_rate`). Uses fixed memory of approximately:
///   `-n * ln(fpr) / (ln(2)^2)` bits, where `n` is `expected_unique_tokens`.
///
/// Ideal for very large corpora where exact dedup would exceed available RAM
/// and a small false-positive rate (≤ 0.1%) is acceptable.
pub fn bloom_stream_dedupe(
    expected_unique_tokens: u64,   // estimated unique count for sizing
    false_positive_rate: f64,      // e.g. 0.001 for 0.1%
    config: &Config,
    cancel: &impl CancelCheck,
    sink: &impl ProgressSink,
) -> Result<Stats>
```

### Core Implementation Sketch

```rust
// crates/core/src/bloom.rs

struct BloomFilter {
    bits: Vec<u64>,  // bit array
    k: usize,        // number of hash functions
    m: usize,        // number of bits
}

impl BloomFilter {
    fn new(n: u64, fpr: f64) -> Self {
        // m = ceil(-n * ln(fpr) / (ln 2)^2)
        // k = ceil(m / n * ln 2)
        let m = (-(n as f64) * fpr.ln() / std::f64::consts::LN_2.powi(2)).ceil() as usize;
        let k = ((m as f64 / n as f64) * std::f64::consts::LN_2).ceil() as usize;
        BloomFilter { bits: vec![0u64; m.div_ceil(64)], k, m }
    }

    /// Returns `true` if token is probably new; `false` if probably seen.
    fn test_and_set(&mut self, token: &str) -> bool {
        let h1 = ahash::AHasher::default().hash_one(token);
        let h2 = ahash::AHasher::default().hash_one(&(token, 0xDEAD_BEEFu64));
        let mut new = false;
        for i in 0..self.k {
            let bit = ((h1.wrapping_add((i as u64).wrapping_mul(h2))) as usize) % self.m;
            let (word, offset) = (bit / 64, bit % 64);
            if self.bits[word] & (1 << offset) == 0 { new = true; }
            self.bits[word] |= 1 << offset;
        }
        new
    }
}

pub fn bloom_stream_dedupe(
    expected: u64,
    fpr: f64,
    config: &Config,
    cancel: &impl CancelCheck,
    sink: &impl ProgressSink,
) -> Result<Stats> {
    let mut bloom = BloomFilter::new(expected, fpr);
    let mut writer = OutputWriter::new(&config.output, &config.output_separator)?;
    let mut tokens_seen = 0u64;
    let mut unique_tokens = 0u64;

    for path in &config.inputs {
        let file = BufReader::new(File::open(path)?);
        let mut reader = LossyLineReader::new(file);
        let mut line = String::new();

        while reader.read_line(&mut line)? > 0 {
            for token in TokenIter::new(&line) {
                tokens_seen += 1;
                if bloom.test_and_set(token) {
                    writer.write_token(token)?;
                    unique_tokens += 1;
                }
                if tokens_seen % 8_192 == 0 {
                    sink.on_event(ProgressEvent::TokensSeen(tokens_seen));
                    ensure_not_canceled(cancel)?;
                }
            }
            line.clear();
        }
    }

    Ok(Stats { tokens_seen, unique_tokens, duplicates: tokens_seen - unique_tokens, ..Default::default() })
}
```

### Why This Design

| Choice | Reason |
|--------|--------|
| No external crate | A minimal Bloom filter is ~50 lines; avoids a heavy dependency for a well-understood structure |
| Double-hashing (Kirsch-Mitzenmacher) | Single-evaluation of h1 + i·h2 simulates k independent hash functions with near-zero cost |
| `u64` word array | CPU-native width; bit operations are single instructions; avoids usize/byte mismatches |
| `expected_unique_tokens` parameter | Caller-supplied estimate allows correct sizing — too low → higher FPR, too high → wasted memory |
| Reuses `OutputWriter` + `LossyLineReader` | Single-pass streaming fits directly in the existing pipeline |

### Memory Cost at 0.1% FPR

| Expected unique tokens | Bloom filter size |
|------------------------|-------------------|
| 1M | ~1.8 MB |
| 10M | ~17.9 MB |
| 100M | ~179 MB |
| 1B | ~1.79 GB |

Compare to exact RAM dedup at 1B tokens: ~20–40 GB for the HashSet alone.

### Use Cases

- **Web crawl deduplication** — single-pass over URL lists or crawled text at petabyte scale
- **Log stream processing** — deduplicate live log tokens as they arrive without buffering
- **Incremental corpus update** — load bloom state from a checkpoint, stream new data through it
- **Memory-constrained environments** — embedded systems, containers with strict memory limits

---

## Summary Table

| # | Function | Module | Mode | Key Benefit | Complexity |
|---|----------|--------|------|-------------|------------|
| 1 | `token_frequency` | `frequency.rs` | RAM | Frequency map sorted by count | O(n) time, O(u) space |
| 2 | `set_op` (Diff/Intersect/Union) | `set_ops.rs` | RAM | Relational operations across file groups | O(n) time, O(u) space |
| 3 | `fuzzy_cluster` | `fuzzy.rs` | RAM (post-dedup) | Near-duplicate clustering by edit distance | O(u²) on unique set |
| 4 | `ngram_extract` | `ngram.rs` | RAM | Deduplicated n-gram corpus generation | O(n) time, O(u) space |
| 5 | `bloom_stream_dedupe` | `bloom.rs` | Single-pass streaming | Sub-linear memory, configurable FPR | O(n) time, O(m) bits |

**Recommended integration order:**
1. `token_frequency` — highest immediate utility, lowest risk, trivially fits CLI/UI
2. `set_op` — directly fills the most common data-science request ("diff two lists")
3. `ngram_extract` — self-contained, no new deps
4. `bloom_stream_dedupe` — no new deps, biggest perf win for large corpora
5. `fuzzy_cluster` — most complex (O(u²)), ship after the others are stable

---

*All five functions reuse `LossyLineReader`, `TokenIter`, `CancelCheck`, `ProgressSink`, and `OutputWriter` from the existing `crates/core` public API. No new dependencies are required for proposals 1, 2, 4, and 5. Proposal 3 (`fuzzy_cluster`) is self-contained but an optional `edit-distance` crate (e.g. `levenshtein` 1.0, 200 lines) could replace the hand-rolled DP for brevity.*

---

## Implementation Plan — Detailed Analysis

> **Basis:** Full source read of every `.rs` file in `crates/core/src/` plus `Cargo.toml`.
> All line references are to the files as they exist at v2.6.3.
> **No new Cargo dependencies are required for any of the five functions.**

---

### Preliminary: What the Real Code Actually Does

Before planning, these facts from the source code must be respected — they contradict or refine several assumptions in the proposals above.

| Fact | File:Line | Impact on proposals |
|------|-----------|---------------------|
| `LossyLineReader::read_line` calls `out.clear()` before writing | `text_line_reader.rs:21` | Do NOT manually clear `line` inside the token loop — that would truncate mid-iteration |
| `ProgressEvent::FileStarted` takes `{ index: usize, total: usize }` | `progress.rs:4` | Proposed sketches used `{ path: PathBuf }` — **wrong** |
| `ProgressEvent::Stage` takes `&'static str` | `progress.rs:3` | Stage strings must be string literals, not heap `String`s |
| `ensure_not_canceled` is `pub(crate)` | `cancel.rs:52` | Accessible from all modules inside `dedupe_core` — no issue |
| `OutputWriter::create` takes `sep: String` (owned) | `writer.rs:12` | Must clone or own the separator |
| `RamStore::insert` takes `&str` | `dedupe_ram.rs:27` | `Box<str>` is allocated inside `insert()` only for new tokens |
| `Config::validate()` rejects empty `output` path | `config.rs:98-100` | Functions that return data (not files) must NOT call `validate()` |
| `hashbrown::HashMap` is in scope | `Cargo.toml:9` | Use `hashbrown::HashMap<K, V, RandomState>`, not `std::collections::HashMap` |
| `indexmap::IndexSet` is in scope | `Cargo.toml:10` | Needed for ordered set operations |
| `ahash` 0.8 `RandomState::hash_one` is available | `Cargo.toml:8` | Use `state.hash_one(value) -> u64` |
| `Stats` derives `Default` | `stats.rs:23` | `..Default::default()` works in struct-update syntax |

---

### fn1 — `token_frequency`: Implementation Plan

**Target file:** `crates/core/src/frequency.rs` (new) + wiring in `lib.rs`

**Corrected signature (after source analysis)**

```rust
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

pub fn token_frequency<P: ProgressSink, C: CancelCheck>(
    config: &Config,
    progress: &P,
    cancel: &C,
) -> anyhow::Result<Vec<(Box<str>, u64)>>
```

**Step 1 — Input guard (no `validate()`).**
`Config::validate()` rejects an empty `output` path. Since `token_frequency` never writes a file, do NOT call it. Instead add a local guard:

```rust
if config.inputs.is_empty() {
    return Ok(Vec::new());
}
```

**Step 2 — Allocate the frequency map.**
Use `hashbrown::HashMap<Box<str>, u64, RandomState>`:

```rust
let mut counts: HashMap<Box<str>, u64, RandomState> =
    HashMap::with_hasher(RandomState::new());
counts.reserve(16 * 1024);  // same initial hint as RamStore in engine.rs:179
```

Using `Box<str>` (not `String`) as key matches `RamStore`'s key type and avoids the extra capacity word.

**Step 3 — Tokenisation loop (mirror `run_ram` exactly).**
The loop structure in [engine.rs:191-278](../crates/core/src/engine.rs) is the reference. Key points:

- `let mut line = String::new();` declared OUTSIDE the file loop
- `reader.read_line(&mut line)` — `LossyLineReader` clears `line` at the start of every call (`text_line_reader.rs:21`). **Do not add a manual clear.**
- Cancellation check every 8 192 tokens (`tokens_seen % 8_192 == 0`)
- Progress event every 100 000 tokens (`tokens_seen % 100_000 == 0`)
- Apply `config.trim`, `config.drop_empty`, `config.should_drop_by_length()` in that order (matching `engine.rs:225-241`)

For insertion into the map, avoid the `entry(token.into())` anti-pattern:

```rust
// entry(token.into()) allocates Box<str> on every call, even for existing keys.
// Use get_mut + insert instead:
if let Some(v) = counts.get_mut(token) {
    *v += 1;
} else {
    counts.insert(token.into(), 1);
}
```

**Step 4 — Sort and return.**

```rust
let mut vec: Vec<(Box<str>, u64)> = counts.into_iter().collect();
// Primary: descending count. Secondary: ascending token (deterministic ties).
vec.sort_unstable_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
Ok(vec)
```

**Step 5 — Wire into `lib.rs`.**

```rust
// lib.rs — add:
pub mod frequency;
pub use frequency::token_frequency;
```

**fn1 — Identified problems**

| # | Problem | Severity | Fix |
|---|---------|----------|-----|
| P1 | `counts.entry(token.into())` allocates `Box<str>` on every call, even for duplicate keys | Perf | Use `get_mut` + `insert` two-step |
| P2 | Calling `Config::validate()` would reject configs with empty output path | Correctness | Skip validate; add minimal local guard |
| P3 | `ProgressEvent::FileStarted { path }` does not exist | Compile error | Use `{ index: idx+1, total: config.inputs.len() }` |
| P4 | `line.clear()` inside inner token loop (original sketch) | Runtime bug | Remove — `LossyLineReader::read_line` owns the clear |
| P5 | `std::collections::HashMap` uses SipHash, not AHash | Perf | Use `hashbrown::HashMap<_, _, RandomState>` |

**fn1 — Test plan**

```rust
// tests/frequency_test.rs (or inline in frequency.rs)
// 1. Empty inputs → Ok(vec![])
// 2. Single token repeated N times → count == N
// 3. Sort order: highest count first; ties broken alphabetically
// 4. trim=true strips whitespace from token before counting
// 5. drop_length_min/max filtering reduces output
// 6. Cancellation mid-way returns Err(Canceled)
```

---

### fn2 — `set_op` (Diff / Intersect / Union): Implementation Plan

**Target file:** `crates/core/src/set_ops.rs` (new) + wiring in `lib.rs`

**Structural problem with the original proposal**

The original sketch passed `left: &[&Path]` and `right: &[&Path]` alongside a `config: &Config`. But `Config.inputs` already holds one list of paths. This creates ambiguity. Two clean approaches:

Option A — Accept two path slices + a filter-only config view (requires a new `TokenFilter` struct).

Option B — Reuse `Config` for the left side; accept right paths separately:

```rust
pub fn set_op(
    config: &Config,       // config.inputs = left group; config.output = destination
    right:  &[PathBuf],
    op:     SetOp,
    progress: &P,
    cancel:   &C,
) -> anyhow::Result<Stats>
```

**Recommendation: Option B.** It is the least invasive API change. `config.inputs` = left set, new `right` parameter = right set. All filter settings come from the single `Config`. `config.validate()` can be called normally since `config.output` is set.

**Corrected signature**

```rust
pub enum SetOp {
    /// Tokens in `config.inputs` that are NOT in `right`.
    Diff,
    /// Tokens present in BOTH `config.inputs` and `right`.
    Intersect,
    /// All unique tokens across both groups (equivalent to merging all inputs).
    Union,
}

pub fn set_op<P: ProgressSink, C: CancelCheck>(
    config:   &Config,
    right:    &[PathBuf],
    op:       SetOp,
    progress: &P,
    cancel:   &C,
) -> anyhow::Result<Stats>
```

**Step 1 — Build two sets using a shared helper.**
Extract a private `collect_token_set` function that mirrors the tokenisation loop of `run_ram` but returns a `HashSet<Box<str>, RandomState>`:

```rust
fn collect_token_set<P: ProgressSink, C: CancelCheck>(
    inputs:       &[PathBuf],
    config:       &Config,
    progress:     &P,
    cancel:       &C,
    index_offset: usize,   // for correct FileStarted index reporting
    total_files:  usize,
) -> anyhow::Result<HashSet<Box<str>, RandomState>>
```

Note the `index_offset` parameter: when building the right set, file indices continue from where the left set left off (for meaningful progress events).

**Step 2 — Apply the set operation.**

```rust
let left_set  = collect_token_set(&config.inputs, config, progress, cancel, 0, total)?;
let right_set = collect_token_set(right, config, progress, cancel, config.inputs.len(), total)?;

let result: Vec<Box<str>> = match op {
    SetOp::Diff => left_set.into_iter()
        .filter(|t| !right_set.contains(t.as_ref()))
        .collect(),
    SetOp::Intersect => left_set.into_iter()
        .filter(|t| right_set.contains(t.as_ref()))
        .collect(),
    SetOp::Union => {
        let mut u = left_set;
        for t in right_set { u.insert(t); }
        u.into_iter().collect()
    }
};
```

`Box<str>` implements `Borrow<str>`, so `.contains(t.as_ref())` avoids allocating a `Box<str>` for the lookup.

**Step 3 — Ordering.**
`PreserveFirstSeen` is not properly supported when collecting into a `HashSet` (unordered). Solutions:

- For `PreserveFirstSeen`: use `IndexSet<Box<str>, RandomState>` (from `indexmap`) for the left set, which preserves insertion order; build right set as plain `HashSet` for O(1) lookups. For `Union`: build both as `IndexSet`, append right items not in left.
- For `Alphabetical`: collect into `Vec`, call `sort_unstable()`.
- For `UnorderedFast`: no action, emit as-is.

**Step 4 — Write output and return `Stats`.**

```rust
let mut out = OutputWriter::create(&config.output, config.output_separator.clone())?;
for token in &result {
    out.write_token(token)?;
}
out.finish()?;
// Populate Stats
```

**fn2 — Identified problems**

| # | Problem | Severity | Fix |
|---|---------|----------|-----|
| P1 | `Config.inputs` conflates left-set inputs with job config | Design | Use Option B: `config.inputs` = left, separate `right: &[PathBuf]` |
| P2 | `HashSet` loses insertion order — `PreserveFirstSeen` silently wrong | Correctness | Use `IndexSet` for left set when `config.ordering == PreserveFirstSeen` |
| P3 | `hashbrown::HashSet::difference()` returns `&T`; filter approach is cleaner for `Box<str>` | API misuse | Use `.filter(|t| !right_set.contains(t.as_ref()))` pattern |
| P4 | Union with `PreserveFirstSeen`: right-only items have no defined position | Spec gap | Document: right-only items append after all left items |
| P5 | Two sets in RAM simultaneously = 2× memory vs. normal dedup | Memory | Document limitation; disk-scale version is future work |
| P6 | Progress `FileStarted` indices span both sets | UX | Pass `index_offset = config.inputs.len()` when scanning right set |

**fn2 — Test plan**

```rust
// 1. Diff(["a","b","c"], ["b","c","d"]) == ["a"]
// 2. Intersect(["a","b"], ["b","c"]) == ["b"]
// 3. Union(["a","b"], ["b","c"]) == {"a","b","c"} (as set)
// 4. Diff with empty right == left (identity)
// 5. Intersect with empty right == [] (empty)
// 6. PreserveFirstSeen ordering preserved for Diff
// 7. Alphabetical ordering on result
// 8. Cancellation mid-way returns Err(Canceled)
```

---

### fn3 — `fuzzy_cluster`: Implementation Plan

**Target file:** `crates/core/src/fuzzy.rs` (new) + wiring in `lib.rs`

**Critical complexity problem with the original proposal**

The original sketch proposes O(u²) naive pairwise comparison. With u=100k unique tokens, that is **5 × 10⁹ comparisons**. Even with an early-exit Levenshtein at `max_edit+1`, this is ~10–60 seconds on a modern CPU. The function is only safe for u ≤ ~10k without optimisation.

**Better approach: length-bucket pruning.**
Tokens with edit distance ≤ `d` can only differ in length by at most `d`. Therefore:

1. Group tokens by character length into buckets `B[len]`.
2. For each token of length `L`, only compare against tokens in `B[L-d]` through `B[L+d]`.
3. This reduces comparisons to `u × avg_bucket_size × (2d+1)` which for typical corpora (Zipfian distribution, narrow length range) is O(u) in practice.

**Revised signature**

```rust
pub fn fuzzy_cluster<C: CancelCheck>(
    tokens:            &[Box<str>],   // already exact-deduped, sorted preferred
    max_edit_distance: usize,         // must be 1 or 2; reject higher values
    cancel:            &C,
) -> anyhow::Result<Vec<Box<str>>>
```

Progress sink is omitted — `fuzzy_cluster` runs on an already-loaded `Vec` with no I/O. A cancellation check every 8 192 comparisons is sufficient.

**Step 1 — Input validation.**

```rust
if max_edit_distance == 0 {
    return Ok(tokens.to_vec());  // nothing to cluster
}
if max_edit_distance > 2 {
    anyhow::bail!("max_edit_distance > 2 is not supported (complexity too high)");
}
```

Hard-capping at 2 is a deliberate safety fence. Edit distance 3+ grows the candidate set dramatically.

**Step 2 — Length-bucket index.**

```rust
use std::collections::BTreeMap;
let mut by_len: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
for (i, t) in tokens.iter().enumerate() {
    by_len.entry(t.chars().count()).or_default().push(i);
}
```

Use `chars().count()` not `len()` — edit distance is on Unicode codepoints, not bytes.

**Step 3 — Union-Find with iterative path compression.**

```rust
let n = tokens.len();
let mut parent: Vec<usize> = (0..n).collect();
let mut rank:   Vec<u8>    = vec![0; n];

fn find(parent: &mut Vec<usize>, mut x: usize) -> usize {
    while parent[x] != x {
        parent[x] = parent[parent[x]];  // path halving — iterative, no recursion
        x = parent[x];
    }
    x
}

fn union(parent: &mut Vec<usize>, rank: &mut Vec<u8>, a: usize, b: usize) {
    let (ra, rb) = (find(parent, a), find(parent, b));
    if ra == rb { return; }
    match rank[ra].cmp(&rank[rb]) {
        std::cmp::Ordering::Less    => parent[ra] = rb,
        std::cmp::Ordering::Greater => parent[rb] = ra,
        std::cmp::Ordering::Equal   => { parent[rb] = ra; rank[ra] += 1; }
    }
}
```

**Path halving** (not full path compression) is iterative and avoids any recursion depth concern.

**Step 4 — Pairwise comparison with length-bucket pruning.**

```rust
let mut comparisons: u64 = 0;
for (&len, indices) in &by_len {
    for d in 0..=max_edit_distance {
        for &cand_len in &[len.saturating_sub(d), len + d] {
            if cand_len == len && d > 0 { continue; }
            if let Some(cands) = by_len.get(&cand_len) {
                for &i in indices {
                    for &j in cands {
                        if i >= j { continue; }   // avoid double-comparison
                        comparisons += 1;
                        if comparisons % 8_192 == 0 { ensure_not_canceled(cancel)?; }
                        if levenshtein_chars(&tokens[i], &tokens[j]) <= max_edit_distance {
                            union(&mut parent, &mut rank, i, j);
                        }
                    }
                }
            }
        }
    }
}
```

Note: same-length pairs must be compared when `d==0` as well (substitutions preserve length). Refine the loop to avoid double-counting within same-length buckets.

**Step 5 — Extract one representative per cluster.**

```rust
let mut best: HashMap<usize, usize, RandomState> =
    HashMap::with_hasher(RandomState::new());

for (i, _) in tokens.iter().enumerate() {
    let root = find(&mut parent, i);
    let entry = best.entry(root).or_insert(i);
    if tokens[i] < tokens[*entry] {
        *entry = i;
    }
}

let mut result: Vec<Box<str>> = best.values()
    .map(|&i| tokens[i].clone())
    .collect();
result.sort_unstable();
Ok(result)
```

**Step 6 — Levenshtein on Unicode codepoints.**

```rust
fn levenshtein_chars(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (m, n) = (a.len(), b.len());
    if m.abs_diff(n) > 2 { return m.abs_diff(n); }  // early exit
    let mut row: Vec<usize> = (0..=n).collect();
    for i in 1..=m {
        let mut prev = row[0];
        row[0] = i;
        for j in 1..=n {
            let temp = row[j];
            row[j] = if a[i-1] == b[j-1] { prev }
                     else { 1 + prev.min(row[j]).min(row[j-1]) };
            prev = temp;
        }
    }
    row[n]
}
```

Allocating `Vec<char>` per comparison is expensive for high call counts. For `max_edit_distance ≤ 2`, a bounded Levenshtein that exits early once the running minimum exceeds the threshold reduces average cost significantly.

**fn3 — Identified problems**

| # | Problem | Severity | Fix |
|---|---------|----------|-----|
| P1 | O(u²) naive comparison blows up for u > 10k | Perf / UX | Length-bucket pruning (plan above) |
| P2 | Original `find()` was recursive — stack overflow on large u with degenerate chains | Safety | Use iterative path halving (plan above) |
| P3 | Edit distance on bytes not chars — `"café"` vs `"cafe"` = 2 bytes but 1 codepoint | Correctness | Use `chars().collect::<Vec<_>>()` |
| P4 | `Vec<char>` allocation per comparison call | Perf | For `max_edit ≤ 2` use bounded inline DP without Vec allocation |
| P5 | No cap on `max_edit_distance` — user passes 5, O(u²) computation with wide windows | Safety | Hard error for `d > 2` |
| P6 | `find()` with mutable borrow needed inside the comparison loop — borrow checker conflict | Compile error | Separate `find` calls from the comparison loop (find roots first, then compare) |
| P7 | Same-length pairs: naive loop double-counts pairs within same bucket | Logic bug | Guard with `i < j` only; handle same-bucket and cross-bucket separately |

**fn3 — Test plan**

```rust
// 1. Exact duplicates already removed → no change
// 2. ["teh","the"] with d=1 → ["the"] (smaller representative)
// 3. ["colour","color"] with d=2 → one representative
// 4. ["colour","color"] with d=1 → two separate (distance=2 > threshold)
// 5. Transitive chain: ["ab","ac","bc"] with d=1 → one cluster
// 6. max_edit_distance=0 → same as input
// 7. max_edit_distance=3 → Err
// 8. Unicode: ["café","cafe"] with d=1 → one cluster
// 9. Empty input → Ok([])
```

---

### fn4 — `ngram_extract`: Implementation Plan

**Target file:** `crates/core/src/ngram.rs` (new) + wiring in `lib.rs`

**Corrected signature**

```rust
pub fn ngram_extract<P: ProgressSink, C: CancelCheck>(
    n:        usize,
    config:   &Config,
    progress: &P,
    cancel:   &C,
) -> anyhow::Result<Stats>
```

`Config` is reused fully — `config.output` is the destination, `config.inputs` are sources, all filter fields apply to individual tokens BEFORE forming n-grams, `config.ordering` applies to the final n-gram set.

**Step 1 — Validate `n`.**

```rust
if n < 2 {
    anyhow::bail!("n must be >= 2 for ngram_extract; use run() for unigrams");
}
config.validate()?;
```

**Step 2 — Window type.**
The window slides over `&str` slices from `TokenIter`. But `&str` borrows from `line`, and `LossyLineReader::read_line` clears `line` on the next call. Therefore the window must hold **owned** strings:

```rust
use std::collections::VecDeque;
let mut window: VecDeque<Box<str>> = VecDeque::with_capacity(n);
```

Use `Box<str>` not `String` — same size, no extra capacity word, matches `RamStore` key type.

**Step 3 — Store for n-gram deduplication.**

```rust
let mut store = match config.ordering {
    OutputOrdering::UnorderedFast => RamStore::new_unordered(),
    OutputOrdering::PreserveFirstSeen | OutputOrdering::Alphabetical => RamStore::new_stable(),
};
store.reserve(16 * 1024);
```

This is the identical pattern as `engine.rs:174-179`.

**Step 4 — Main loop.**

```rust
for (idx, path) in config.inputs.iter().enumerate() {
    ensure_not_canceled(cancel)?;
    progress.on_event(ProgressEvent::FileStarted { index: idx + 1, total: config.inputs.len() });
    let file = File::open(path)?;
    let mut reader = LossyLineReader::new(BufReader::new(file));
    let mut line = String::new();
    loop {
        ensure_not_canceled(cancel)?;
        let bytes = reader.read_line(&mut line)?;
        if bytes == 0 { break; }
        for raw in TokenIter::new(&line) {
            let mut token = raw;
            if config.trim { token = token.trim(); }
            if config.drop_empty && token.is_empty() { continue; }
            if config.should_drop_by_length(token) { stats.filtered_by_length += 1; continue; }
            stats.tokens_seen += 1;
            window.push_back(token.into());   // Box<str> — necessary: &str borrows from `line`
            if window.len() == n {
                let gram = window.iter().map(|t| t.as_ref()).collect::<Vec<&str>>().join(" ");
                if store.insert(&gram) { stats.unique_tokens += 1; }
                else { stats.duplicates += 1; }
                window.pop_front();
            }
            if stats.tokens_seen % 8_192 == 0 { ensure_not_canceled(cancel)?; }
            if stats.tokens_seen % 100_000 == 0 {
                progress.on_event(ProgressEvent::TokensSeen(stats.tokens_seen));
            }
        }
        window.clear();  // reset at line boundary — prevents cross-sentence n-grams
    }
    progress.on_event(ProgressEvent::FileFinished { index: idx + 1, total: config.inputs.len() });
}
```

**Step 5 — Sorting and output.**
Identical to `run_ram` (`engine.rs:281-298`):

```rust
ensure_not_canceled(cancel)?;
let mut tokens = store.into_tokens();
if matches!(config.ordering, OutputOrdering::Alphabetical) {
    progress.on_event(ProgressEvent::Stage("Sorting"));
    tokens.sort_unstable();
}
progress.on_event(ProgressEvent::Stage("WritingOutput"));
let mut out = OutputWriter::create(&config.output, config.output_separator.clone())?;
for token in tokens {
    ensure_not_canceled(cancel)?;
    out.write_token(&token)?;
}
out.finish()?;
```

**fn4 — Identified problems**

| # | Problem | Severity | Fix |
|---|---------|----------|-----|
| P1 | `window` must hold owned data — `&str` borrows from `line` which is cleared on next `read_line` | Compile error | Use `VecDeque<Box<str>>` with `token.into()` |
| P2 | `Box<str>` clone per window slot × n — at n=3 and 100M tokens: 300M small allocations | Perf | Acceptable; arena allocator is a future optimisation |
| P3 | `Vec<&str>::join(" ")` + `store.insert(&gram)` = two allocations per unique n-gram | Perf | Unavoidable with current `RamStore::insert(&str)` API |
| P4 | `config.mode` is ignored — ngram_extract is always RAM-only | Design | Document explicitly; DISK-mode n-gram extraction is future work |
| P5 | `stats.tokens_seen` counts individual tokens, not n-grams — may confuse callers | Clarity | Document: `tokens_seen` = individual tokens, `unique_tokens` = unique n-grams |
| P6 | Window reset at line boundary discards cross-line n-grams | Design choice | Intentional and correct for sentence-aware NLP; document it |
| P7 | `n = 1` should redirect to `run()` | Correctness | `anyhow::bail!` for `n < 2` |

**fn4 — Test plan**

```rust
// 1. "a b c" with n=2 → ["a b", "b c"]
// 2. Duplicate n-grams deduplicated: "a b a b" → ["a b", "b a"] (PreserveFirstSeen)
// 3. Alphabetical ordering of n-grams
// 4. Window reset at line boundary: "a b\nc d" with n=2 → ["a b","c d"] (NOT "b c")
// 5. n=1 → Err
// 6. filter interactions: drop_empty drops tokens before window
// 7. Multi-file: window resets between files too
// 8. stats.unique_tokens == number of unique n-grams written
```

---

### fn5 — `bloom_stream_dedupe`: Implementation Plan

**Target file:** `crates/core/src/bloom.rs` (new) + wiring in `lib.rs`

**Critical issues with the original proposal**

Issue A — `ahash::AHasher` API in 0.8.
In `ahash` 0.8, `AHasher` does not implement `std::hash::Hasher::hash_one`. The method lives on `RandomState`:

```rust
use ahash::RandomState;
let state = RandomState::new();
let h: u64 = state.hash_one(value);
```

For Kirsch-Mitzenmacher double hashing, initialize two separate states once in `BloomFilter::new()`:

```rust
let state1 = RandomState::with_seeds(1, 2, 3, 4);  // fixed seeds → deterministic
let state2 = RandomState::with_seeds(5, 6, 7, 8);
// Per token:
let h1 = state1.hash_one(token) as usize;
let h2 = state2.hash_one(token) as usize;
```

Issue B — `m.div_ceil(64)` MSRV.
`usize::div_ceil` was stabilised in Rust 1.73.0. MSRV is not pinned in `Cargo.toml`. Use the portable form `(m + 63) / 64` to be safe.

Issue C — Output ordering.
Bloom-filter streaming dedup is inherently first-seen-wins. `Alphabetical` ordering is impossible in a single pass — the function must reject it with `anyhow::bail!`.

**Revised `BloomFilter` struct**

```rust
pub struct BloomFilter {
    bits:   Vec<u64>,
    k:      usize,
    m:      usize,
    state1: ahash::RandomState,
    state2: ahash::RandomState,
}

impl BloomFilter {
    pub fn new(expected_n: u64, fpr: f64) -> Self {
        assert!(expected_n > 0, "expected_n must be > 0");
        assert!((0.0..1.0).contains(&fpr) && fpr > 0.0, "fpr must be in (0,1)");
        let m = (-(expected_n as f64) * fpr.ln() / std::f64::consts::LN_2.powi(2))
                .ceil() as usize;
        let m = m.max(64);
        let k = ((m as f64 / expected_n as f64) * std::f64::consts::LN_2).ceil() as usize;
        let k = k.max(1);
        BloomFilter {
            bits:   vec![0u64; (m + 63) / 64],
            k, m,
            state1: ahash::RandomState::with_seeds(1, 2, 3, 4),
            state2: ahash::RandomState::with_seeds(5, 6, 7, 8),
        }
    }

    #[inline]
    pub fn test_and_set(&mut self, token: &str) -> bool {
        let h1 = self.state1.hash_one(token) as usize;
        let h2 = (self.state2.hash_one(token) as usize).max(1);  // guard against h2=0
        let mut is_new = false;
        for i in 0..self.k {
            let bit    = h1.wrapping_add(i.wrapping_mul(h2)) % self.m;
            let mask   = 1u64 << (bit % 64);
            if self.bits[bit / 64] & mask == 0 { is_new = true; }
            self.bits[bit / 64] |= mask;
        }
        is_new
    }
}
```

**Main function signature**

```rust
pub fn bloom_stream_dedupe<P: ProgressSink, C: CancelCheck>(
    expected_unique_tokens: u64,
    false_positive_rate:    f64,
    config:                 &Config,
    progress:               &P,
    cancel:                 &C,
) -> anyhow::Result<Stats>
```

Step 1 — validate: call `config.validate()`, reject `Alphabetical` ordering and invalid FPR.
Step 2 — run: mirror the `run_ram` loop; replace `RamStore::insert` with `bloom.test_and_set`.
Step 3 — stats: document that `stats.duplicates` is approximate (false positives counted as dups).

**Memory budget at various FPR values**

```text
m = -n * ln(fpr) / (ln 2)²

fpr=0.001 (0.1%):  m/n ≈ 14.38 bits/token
fpr=0.01  (1%):    m/n ≈  9.59 bits/token
fpr=0.1   (10%):   m/n ≈  4.79 bits/token
```

| Expected unique | FPR 0.1% | FPR 1% | FPR 10% |
|-----------------|----------|--------|---------|
| 1M | 1.80 MB | 1.20 MB | 0.60 MB |
| 10M | 17.9 MB | 11.9 MB | 5.97 MB |
| 100M | 179 MB | 119 MB | 59.7 MB |
| 1B | 1.79 GB | 1.19 GB | 597 MB |

**fn5 — Identified problems**

| # | Problem | Severity | Fix |
|---|---------|----------|-----|
| P1 | `AHasher::hash_one` does not exist in ahash 0.8 — must use `RandomState::hash_one` | Compile error | Use `self.state1.hash_one(token) as usize` |
| P2 | `m.div_ceil(64)` is MSRV 1.73 — not pinned in Cargo.toml | Minor | Use `(m + 63) / 64` for portability |
| P3 | `Alphabetical` ordering impossible in streaming mode | Design gap | Reject with `anyhow::bail!` |
| P4 | False positives silently inflate `stats.duplicates` | Accuracy | Document: stats are approximate when using Bloom filter |
| P5 | `expected_unique_tokens` is hard to estimate — wrong estimate changes actual FPR | UX | Provide helper `bloom_sizing(n, fpr) -> (m_bits, k)` so callers can verify |
| P6 | No way to persist or resume the filter state | Feature gap | Out of scope for v1; document as future work |
| P7 | `h2 = 0` in Kirsch-Mitzenmacher causes all probes to land on the same bit | Correctness | Guard: `let h2 = h2.max(1)` (already in revised struct above) |

**fn5 — Test plan**

```rust
// 1. All unique tokens → all written (no false negatives with correct impl)
// 2. Exact duplicate stream → ~0 duplicates pass through (within FPR tolerance)
// 3. stats.tokens_seen correct for mixed stream
// 4. Alphabetical ordering → Err
// 5. fpr=0.0 → Err; fpr=1.0 → Err
// 6. expected_unique_tokens=0 → panic (assert)
// 7. Single-token repeated 1M times → exactly 1 written
// 8. h2=0 degenerate case handled correctly by .max(1) guard
```

---

### Wiring Plan: `lib.rs` Changes

Five new module declarations and re-exports are required. The full diff to [crates/core/src/lib.rs](../crates/core/src/lib.rs):

```rust
// Add these pub mod declarations:
pub mod bloom;
pub mod frequency;
pub mod fuzzy;
pub mod ngram;
pub mod set_ops;

// Add these pub use re-exports:
pub use bloom::{bloom_stream_dedupe, BloomFilter};
pub use frequency::token_frequency;
pub use ngram::ngram_extract;
pub use set_ops::{set_op, SetOp};
pub use fuzzy::fuzzy_cluster;
```

Note: `SetOp` enum belongs in `set_ops.rs`, not `fuzzy.rs` — the original proposal incorrectly placed it.

---

### No New Dependencies Required

All five functions are implementable with the **current `Cargo.toml`** dependencies:

| Dependency needed | Already in Cargo.toml? |
|-------------------|------------------------|
| `hashbrown::HashMap` | Yes (`hashbrown = "0.14"`) |
| `ahash::RandomState` | Yes (`ahash = "0.8"`) |
| `indexmap::IndexSet` | Yes (`indexmap = "2"`) |
| `std::collections::VecDeque` | stdlib |
| `std::collections::BinaryHeap` | stdlib (already used in `disk_sort.rs`) |
| Levenshtein DP | Hand-rolled, no dep |
| Bloom filter bit array | Hand-rolled, no dep |

---

### Recommended Implementation Order (Revised)

| Priority | Function | Reason |
|----------|----------|--------|
| 1 | `token_frequency` | Simplest: one loop, no new data structures, reuses existing patterns exactly |
| 2 | `ngram_extract` | Self-contained, `VecDeque` window is the only new concept |
| 3 | `set_op` | Requires careful `IndexSet` vs `HashSet` selection per ordering mode |
| 4 | `bloom_stream_dedupe` | Requires `BloomFilter` struct, parameter validation, ordering restriction |
| 5 | `fuzzy_cluster` | Most complex: Union-Find + length-bucketing + Unicode Levenshtein |
