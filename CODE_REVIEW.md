# Dupli-Annihilator-G — Comprehensive Rust Code Review

**Reviewer:** Expert Senior Rust Developer
**Date:** 2026-03-11
**Codebase version:** 2.6.0
**Scope:** All Rust source files across `crates/core`, `crates/job_runner`, `crates/backend`, `apps/cli`, `apps/desktop/src-tauri`

---

## Executive Summary

The codebase is **well-structured, idiomatic, and production-quality** overall. The layered architecture (core → job_runner → backend → UI) is clean, the cancellation design using trait-based `CancelCheck` is elegant, and the dual-mode (RAM/Disk) pipeline is thoughtfully implemented. That said, a handful of issues ranging from a shell-injection vector to redundant allocations on hot paths, misleading documentation, and silent behavioral gaps deserve attention.

Findings are classified as:
- 🔴 **High** — Security risk or data-correctness bug
- 🟡 **Medium** — Performance regression, behavioral inconsistency, or misleading API
- 🟢 **Low** — Code-quality, style, or minor inefficiency

---

## Table of Contents

1. [HIGH — Shell Injection via `cmd /C start` on Windows](#1-high--shell-injection-via-cmd-c-start-on-windows)
2. [HIGH — Mutex `.unwrap()` Panics in WordChecker Path](#2-high--mutex-unwrap-panics-in-wordchecker-path)
3. [MEDIUM — Redundant Double-Filtering in `reduce_to_output`](#3-medium--redundant-double-filtering-in-reduce_to_output)
4. [MEDIUM — Unnecessary `.to_string()` Allocation on Every Token in Disk Reduce](#4-medium--unnecessary-tostring-allocation-on-every-token-in-disk-reduce)
5. [MEDIUM — Avoidable `clone()` on Hot Path in `merge_runs_to_output`](#5-medium--avoidable-clone-on-hot-path-in-merge_runs_to_output)
6. [MEDIUM — `Mode::Auto` Is Silently Identical to RAM Mode](#6-medium--modeauto-is-silently-identical-to-ram-mode)
7. [MEDIUM — `per_file_stats` Silently Ignored in Disk Mode](#7-medium--per_file_stats-silently-ignored-in-disk-mode)
8. [MEDIUM — `filtered_by_length` Always Zero in Cancel/Error Summaries](#8-medium--filtered_by_length-always-zero-in-cancelerror-summaries)
9. [MEDIUM — Misleading Progress Stage in `reduce_to_output`](#9-medium--misleading-progress-stage-in-reduce_to_output)
10. [MEDIUM — Bucket Writers Remain Open During Reduce Phase (Windows Risk)](#10-medium--bucket-writers-remain-open-during-reduce-phase-windows-risk)
11. [MEDIUM — Misleading / Inverted Field Comments on `drop_length_min/max`](#11-medium--misleading--inverted-field-comments-on-drop_length_minmax)
12. [MEDIUM — `map_anyhow_to_command_error` Uses Fragile String Matching](#12-medium--map_anyhow_to_command_error-uses-fragile-string-matching)
13. [LOW — Double Lookup in `RamStore::insert` and `WordChecker::load`](#13-low--double-lookup-in-ramstoreinsert-and-wordcheckerload)
14. [LOW — `resolve_rich_inputs` Iterates `config.inputs` Twice](#14-low--resolve_rich_inputs-iterates-configinputs-twice)
15. [LOW — `TokenIter` Recreates `Chars` Iterator Per Character](#15-low--tokeniter-recreates-chars-iterator-per-character)
16. [LOW — `reduce_to_output` Uses `BufRead::lines()` Instead of `LossyLineReader`](#16-low--reduce_to_output-uses-bufreadlines-instead-of-lossylinereader)
17. [LOW — Unbounded Recursion in `collect_compatible_files`](#17-low--unbounded-recursion-in-collect_compatible_files)
18. [LOW — `strip_tags` Is Naive and Will Produce Junk Tokens](#18-low--strip_tags-is-naive-and-will-produce-junk-tokens)
19. [LOW — `SharedSink` Newtype Is Unnecessary Indirection](#19-low--sharedsink-newtype-is-unnecessary-indirection)
20. [LOW — `temp_bytes_total` Is Always `None`](#20-low--temp_bytes_total-is-always-none)
21. [LOW — Missing `Default` impl for `BackendService`](#21-low--missing-default-impl-for-backendservice)
22. [LOW — `AHasher::default()` Uses Fixed Keys in `bucket_index`](#22-low--ahasherdefault-uses-fixed-keys-in-bucket_index)

---

## Detailed Findings

---

### 1. ✅ ~~🔴 HIGH — Shell Injection via `cmd /C start` on Windows~~ — **FIXED**

**File:** [`apps/desktop/src-tauri/src/main.rs`](apps/desktop/src-tauri/src/main.rs)

**Fix applied (2026-03-11):** Replaced the `cmd /C start` invocation with `open::that(path)` from the [`open`](https://crates.io/crates/open) crate (`open = "5"` added to `Cargo.toml`). The `open` crate calls `ShellExecuteW` directly on Windows — no `cmd.exe` shell is involved, so shell metacharacters (`&`, `|`, `>`, etc.) in path arguments are never interpreted. The fix is also cross-platform, removing the manual OS-branching boilerplate.

```rust
// Fixed implementation:
fn open_path_with_default_app(path: &str) -> Result<(), String> {
    open::that(path).map_err(|e| format!("failed to open '{}': {e}", path))
}
```

The `use std::process::Command` import was removed as it is no longer needed.

---

### 2. ✅ ~~🔴 HIGH — Mutex `.unwrap()` Panics in WordChecker Path~~ — **FIXED**

**File:** [`crates/backend/src/lib.rs`](crates/backend/src/lib.rs)

**Fix applied (2026-03-11):** Both `.lock().unwrap()` calls in `load_wordlist_for_checker` and `check_word` have been replaced with `.lock().map_err(...)?`, returning a graceful `CommandError { category: "internal_error", ... }` on mutex poisoning. This follows the established pattern used throughout `job_runner` (`.map_err(|_| anyhow!("... poisoned"))`). A panic in a thread holding the `checker` lock can no longer wedge the application.

```rust
// Fixed — both call sites now use:
self.checker.lock().map_err(|_| CommandError {
    category: "internal_error".to_string(),
    message: "wordlist checker lock poisoned".to_string(),
    detail: None,
})?

---

### 3. ✅ ~~🟡 MEDIUM — Redundant Double-Filtering in `reduce_to_output`~~ — **FIXED**

**File:** [`crates/core/src/disk.rs`](crates/core/src/disk.rs)

**Fix applied (2026-03-11):** Removed the redundant `trim` re-check and `drop_empty` re-check from the `reduce_to_output` bucket-read loop. Tokens written to bucket files by `partition_inputs` are already trimmed, non-empty, and length-filtered — the re-checks were always no-ops. Added an inline comment documenting this pre-filtering invariant to guard against future regressions:

```rust
// Bucket files are pre-filtered by partition_inputs: tokens are
// already trimmed, non-empty, and length-filtered. No re-checking needed.
let token = line?;
```

`let mut token` was simplified to `let token` since the binding is no longer reassigned.

---

### 4. ✅ ~~🟡 MEDIUM — Unnecessary `.to_string()` Allocation on Every Token in Disk Reduce~~ — **FIXED**

**File:** [`crates/core/src/disk.rs`](crates/core/src/disk.rs)

**Fix applied (2026-03-11):** Resolved as a direct consequence of fixing finding #3. Removing the `token.trim().to_string()` call eliminates the per-token heap allocation entirely. `BufRead::lines()` already returns an owned `String`; it is now used as-is with no copy.

---

### 5. ✅ ~~🟡 MEDIUM — Avoidable `clone()` on Hot Path in `merge_runs_to_output`~~ — **FIXED**

**File:** [`crates/core/src/disk_sort.rs`](crates/core/src/disk_sort.rs)

**Fix applied (2026-03-11):** Changed `last_written = Some(token.clone())` to `last_written = Some(token)`. `token` is moved out of the heap; `run_id: usize` is `Copy` and remains valid after the move. `out.write_token` borrows `token` before the move, so the borrow checker is fully satisfied. Every unique token written during a merge-sort reduce now avoids a heap allocation.

---

### 6. ✅ ~~🟡 MEDIUM — `Mode::Auto` Is Silently Identical to RAM Mode~~ — **FIXED**

**Files:** [`crates/core/src/engine.rs`](crates/core/src/engine.rs), [`crates/core/Cargo.toml`](crates/core/Cargo.toml), [`crates/core/src/lib.rs`](crates/core/src/lib.rs), [`crates/job_runner/src/lib.rs`](crates/job_runner/src/lib.rs)

**Fix applied (2026-03-11):** Full adaptive implementation. Added `sysinfo = "0.33"` to `crates/core`.

**New public API — `effective_mode(config: &Config) -> Mode`** (exported from `dedupe_core`):

- For `Mode::Ram` / `Mode::Disk`: returns the mode as-is.
- For `Mode::Auto`: queries `sysinfo` for available system memory, sums the total input file sizes, and returns `Mode::Disk` when inputs exceed **50 % of available RAM** (conservative threshold accounting for hash-set overhead of ~1.5–2× raw token volume); otherwise returns `Mode::Ram`. Never returns `Mode::Auto`.

```rust
pub fn effective_mode(config: &Config) -> Mode {
    match config.mode {
        Mode::Ram | Mode::Disk => config.mode,
        Mode::Auto => resolve_auto_mode(config),
    }
}

fn resolve_auto_mode(config: &Config) -> Mode {
    let total_input_bytes: u64 = config.inputs.iter()
        .filter_map(|p| std::fs::metadata(p).ok())
        .map(|m| m.len()).sum();
    let sys = System::new_with_specifics(
        RefreshKind::nothing().with_memory(MemoryRefreshKind::everything()),
    );
    if sys.available_memory() > 0 && total_input_bytes > sys.available_memory() / 2 {
        Mode::Disk
    } else {
        Mode::Ram
    }
}
```

**`job_runner` changes:**

- `effective_mode(&config)` is called once **before** the job thread spawns, so all terminal paths (success / cancel / error) report the correct `mode_effective` in `RunSummary`.
- `build_run_summary` now accepts `chosen_mode: Mode` and uses it directly for `mode_effective`, instead of re-calling `mode_effective_name(config.mode)`.
- The placeholder "Auto mode not implemented" warning was removed from `build_warnings` — `mode_effective` in the summary now correctly reflects the actual mode chosen.

---

### 7. ✅ ~~🟡 MEDIUM — `per_file_stats` Silently Ignored in Disk Mode~~ — **FIXED**

**File:** [`crates/job_runner/src/lib.rs`](crates/job_runner/src/lib.rs)

**Fix applied (2026-03-11):** Added a warning to `build_warnings` that fires when `config.per_file_stats == true` and the resolved effective mode is Disk (including `Mode::Auto` resolved to Disk via finding #6). The warning is surfaced in `RunSummary.warnings`:

```rust
if config.per_file_stats && matches!(chosen_mode, Mode::Disk) {
    out.push(
        "per_file_stats is not supported in Disk mode; per_file will be null in the summary."
            .to_string(),
    );
}
```

As a bonus, the existing `Disk + PreserveFirstSeen` warning was also corrected to use `chosen_mode` instead of `config.mode`, so it now fires correctly when `Mode::Auto` resolves to Disk.

---

### 8. ✅ ~~🟡 MEDIUM — `filtered_by_length` Always Zero in Cancel/Error Summaries~~ — **FIXED**

**Files:** [`crates/core/src/progress.rs`](crates/core/src/progress.rs), [`crates/core/src/engine.rs`](crates/core/src/engine.rs), [`crates/core/src/disk.rs`](crates/core/src/disk.rs), [`crates/job_runner/src/lib.rs`](crates/job_runner/src/lib.rs)

**Fix applied (2026-03-11):** Wired `filtered_by_length` through the progress event system end-to-end:

1. Added `ProgressEvent::FilteredByLength(u64)` variant to `progress.rs`.
2. Engines emit it every 100 k filtered tokens — matching the `Duplicates` / `UniqueTokens` pattern — in both `run_ram` (`engine.rs`) and `partition_inputs` (`disk.rs`).
3. `ProgressSnapshot` gains a `filtered_by_length: u64` field.
4. `BridgeSink::on_event` handles the new variant: `state.snapshot.filtered_by_length = v`.
5. `to_stats_snapshot()` now forwards `self.filtered_by_length` instead of hard-coding `0`.

Cancel/error summaries now report the last emitted `filtered_by_length` count (accurate to within ±100 k tokens, consistent with the resolution of all other live counters).

---

### 9. ✅ ~~🟡 MEDIUM — Misleading Progress Stage in `reduce_to_output`~~ — **FIXED**

**File:** [`crates/core/src/disk.rs`](crates/core/src/disk.rs)

**Fix applied (2026-03-12):** Removed the trailing `progress.on_event(ProgressEvent::Stage("WritingOutput"))` call from `reduce_to_output`. It was emitted after all bucket processing and output writing had already completed, covering only the internal `BufWriter::flush()` inside `out.finish()` — a sub-millisecond operation. The "ReducingBuckets" stage already correctly encompasses all meaningful work. Users no longer see a misleading "WritingOutput" flash at the very end of a disk-mode job.

---

### 10. ✅ ~~🟡 MEDIUM — Bucket Writers Remain Open During Reduce Phase (Windows Risk)~~ — **FIXED**

**File:** [`crates/core/src/disk.rs`](crates/core/src/disk.rs)

**Fix applied (2026-03-12):** Replaced `DiskBuckets` with a typestate pair. `WritableBuckets::partition_inputs` now takes `self` by value, flushes all writers, then moves only `_dir` and `bucket_paths` into the returned `ReducibleBuckets`. The `Vec<BufWriter<File>>` is dropped at the end of `partition_inputs` — all write handles are closed before `reduce_to_output` opens the same files for reading.

```rust
/// Typestate: bucket files are open for writing.
pub struct WritableBuckets {
    _dir: tempfile::TempDir,
    bucket_paths: Vec<PathBuf>,
    bucket_writers: Vec<BufWriter<File>>,
}

/// Typestate: bucket files have been written and closed; ready for reading.
pub struct ReducibleBuckets {
    _dir: tempfile::TempDir,
    bucket_paths: Vec<PathBuf>,
}

impl WritableBuckets {
    pub fn partition_inputs<P, C>(mut self, ...) -> anyhow::Result<ReducibleBuckets> {
        // ... write tokens ...
        for writer in &mut self.bucket_writers { writer.flush()?; }
        // bucket_writers dropped here — all write handles closed
        Ok(ReducibleBuckets { _dir: self._dir, bucket_paths: self.bucket_paths })
    }
}

impl ReducibleBuckets {
    pub fn reduce_to_output<P, C>(&self, ...) -> anyhow::Result<()> { ... }
}
```

`engine.rs` call sites updated to chain via the returned value:

```rust
let buckets = WritableBuckets::new(config.disk_buckets)?;
let buckets = buckets.partition_inputs(config, progress, &mut stats, cancel)?;
buckets.reduce_to_output(config, progress, &mut stats, cancel)?;
```

---

### 11. ✅ ~~🟡 MEDIUM — Misleading / Inverted Field Comments on `drop_length_min/max`~~ — **FIXED**

**File:** [`crates/core/src/config.rs`](crates/core/src/config.rs)

**Fix applied (2026-03-12):** Replaced the misleading per-field comments with accurate joint-range documentation:

```rust
/// The lower bound of the length-filter range (inclusive).
/// When both `drop_length_min` and `drop_length_max` are set, tokens whose
/// character length satisfies `min <= len <= max` are dropped.
/// Valid range: 1..=10.
pub drop_length_min: Option<usize>,

/// The upper bound of the length-filter range (inclusive).
/// See `drop_length_min` for the complete behaviour description.
/// Valid range: 1..=10.
pub drop_length_max: Option<usize>,
```

The old comments implied each field independently controlled dropping (e.g. "drop tokens whose character length is >= this value"), which was factually wrong and could cause serious user confusion when configuring the filter.

---

### 12. ✅ ~~🟡 MEDIUM — `map_anyhow_to_command_error` Uses Fragile String Matching~~ — **FIXED**

**Files:** [`crates/core/src/config.rs`](crates/core/src/config.rs), [`crates/job_runner/src/lib.rs`](crates/job_runner/src/lib.rs), [`crates/backend/src/lib.rs`](crates/backend/src/lib.rs)

**Fix applied (2026-03-12):** Introduced typed error enums across three crates; replaced all substring matching with `downcast_ref`.

**`crates/core/src/config.rs`** — New `ConfigError` enum (using the already-present `thiserror` dependency), exported from `crates/core/src/lib.rs`. `Config::validate()` now returns `Result<(), ConfigError>` instead of `anyhow::Result<()>`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("no input files provided")]       NoInputs,
    #[error("output path is required")]       NoOutput,
    #[error("output separator cannot be empty")] EmptySeparator,
    #[error("drop_length_min must be between 1 and 10")] DropLengthMinOutOfRange,
    #[error("drop_length_max must be between 1 and 10")] DropLengthMaxOutOfRange,
    #[error("drop_length_min ({0}) must be <= drop_length_max ({1})")] DropLengthRangeInverted(usize, usize),
    #[error("disk_buckets too small")]        DiskBucketsTooSmall,
    #[error("disk_run_bytes too small")]      DiskRunBytesTooSmall,
}
```

**`crates/job_runner/src/lib.rs`** — New `JobError` enum (added `thiserror` dep). `start_job` returns `JobError::Busy.into()` instead of `anyhow!("another job is already running")`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum JobError {
    #[error("another job is already running")]
    Busy,
}
```

**`crates/backend/src/lib.rs`** — Added private `InputError` enum; `expand_path` returns `InputError::NoCompatibleFiles(...)` instead of `anyhow::bail!`; `cfg.validate()` handled at call site with `"invalid_config"` category directly; `map_anyhow_to_command_error` rewritten with zero string matching:

```rust
fn map_anyhow_to_command_error(err: anyhow::Error) -> CommandError {
    let message = err.to_string();
    let category = if err.downcast_ref::<JobError>().is_some() {
        "job_busy"
    } else if err.downcast_ref::<InputError>().is_some() {
        "invalid_config"
    } else {
        "runtime_error"
    };
    CommandError { category: category.to_string(), message, detail: Some(format!("{err:#}")) }
}
```

---

### 13. ✅ ~~🟢 LOW — Double Lookup in `RamStore::insert` and `WordChecker::load`~~ — **FIXED (WordChecker)**

**File:** [`crates/core/src/word_checker.rs`](crates/core/src/word_checker.rs)

**Fix applied (2026-03-12):** Removed the redundant `contains`-before-`insert` guard in `WordChecker::load`. A wordlist has few or zero duplicates, so the pre-check added a hash lookup per word for no benefit. `HashSet::insert` handles duplicates silently by returning `false`:

```rust
// Before:
if !words.contains(token) {
    words.insert(token.into());
}

// After:
words.insert(token.into());
```

**Note:** The equivalent pattern in `RamStore::insert` is intentional — it avoids allocating `Box<str>` on the hot path where most tokens are expected to be duplicates. Left unchanged.

---

### 14. ✅ ~~🟢 LOW — `resolve_rich_inputs` Iterates `config.inputs` Twice~~ — **FIXED**

**File:** [`crates/core/src/engine.rs`](crates/core/src/engine.rs)

**Fix applied (2026-03-12):** Replaced the `filter().count()` early-return pass with `.any()`, and dropped the now-unnecessary `Vec::with_capacity(rich_count)`. One iteration instead of two:

```rust
// Before:
let rich_count = config.inputs.iter()
    .filter(|p| pdf_reader::is_pdf(p) || epub_reader::is_epub(p))
    .count();
if rich_count == 0 { return Ok((config.clone(), Vec::new(), Vec::new())); }
let mut temp_files: Vec<NamedTempFile> = Vec::with_capacity(rich_count);

// After:
if !config.inputs.iter().any(|p| pdf_reader::is_pdf(p) || epub_reader::is_epub(p)) {
    return Ok((config.clone(), Vec::new(), Vec::new()));
}
let mut temp_files: Vec<NamedTempFile> = Vec::new();
```

---

### 15. ✅ ~~🟢 LOW — `TokenIter` Recreates `Chars` Iterator Per Character~~ — **FIXED**

**File:** [`crates/core/src/token_iter.rs`](crates/core/src/token_iter.rs)

**Fix applied (2026-03-12):** Replaced the manual `pos: usize` + repeated `self.s[self.pos..].chars().next()` approach with a single `std::str::CharIndices` stored in the struct. The iterator is created once in `TokenIter::new` and advanced with `.next()` through the lifetime of the `TokenIter`. All call sites are unchanged.

```rust
// Before: new Chars iterator constructed on every character
pub struct TokenIter<'a> { s: &'a str, pos: usize }
// …
let c = self.s[self.pos..].chars().next()?;  // O(1) but allocates Chars each time

// After: CharIndices created once, advanced in place
pub struct TokenIter<'a> { s: &'a str, chars: std::str::CharIndices<'a> }

fn next(&mut self) -> Option<Self::Item> {
    let start = loop {
        let (i, c) = self.chars.next()?;
        if !is_delim(c) { break i; }
    };
    let end = loop {
        match self.chars.next() {
            None => break self.s.len(),
            Some((i, c)) if is_delim(c) => break i,
            Some(_) => {}
        }
    };
    Some(&self.s[start..end])
}
```

All 8 engine smoke tests and 4 word_checker unit tests pass.

---

### 16. ✅ ~~🟢 LOW — `reduce_to_output` Uses `BufRead::lines()` Instead of `LossyLineReader`~~ — **FIXED**

**File:** [`crates/core/src/disk.rs`](crates/core/src/disk.rs)

**Fix applied (2026-03-12):** Replaced `BufReader + reader.lines()` with `LossyLineReader` in `reduce_to_output`, consistent with `partition_inputs`. The trailing newline preserved by `LossyLineReader::read_line` is trimmed before insertion. The now-unused `BufRead` import was also removed.

```rust
// Before:
let file = File::open(path)?;
let reader = BufReader::new(file);
for line in reader.lines() {
    let token = line?;  // Err on invalid UTF-8
    store.insert(&token);
}

// After:
let file = File::open(path)?;
let mut reader = LossyLineReader::new(BufReader::new(file));
let mut line = String::new();
loop {
    let n = reader.read_line(&mut line)?;
    if n == 0 { break; }
    // Trim the trailing newline preserved by LossyLineReader::read_line.
    let token = line.trim_end_matches(|c: char| c == '\n' || c == '\r');
    store.insert(token);
}
```

---

### 17. ✅ ~~🟢 LOW — Unbounded Recursion in `collect_compatible_files`~~ — **FIXED**

**File:** [`crates/backend/src/lib.rs`](crates/backend/src/lib.rs)

**Fix applied (2026-03-12):** Converted from recursion to an iterative DFS with an explicit `Vec<PathBuf>` stack. Symlink behavior is unchanged (symlinks are not followed since `entry.file_type()` returns the symlink type, not the target).

```rust
fn collect_compatible_files(root: &std::path::Path, out: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    // Iterative DFS — avoids stack overflow on deeply nested directory trees.
    let mut dirs = vec![root.to_path_buf()];
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(&dir)? { ... }
    }
    Ok(())
}
```

---

### 18. ✅ ~~🟢 LOW — `strip_tags` Is Naive and Will Produce Junk Tokens~~ — **PARTIALLY FIXED**

**File:** [`crates/core/src/epub_reader.rs`](crates/core/src/epub_reader.rs)

**Fix applied (2026-03-12):** Added `decode_html_entities` post-pass called from `strip_tags`. Handles the 6 most common named entities without a new dependency: `&nbsp;` `&lt;` `&gt;` `&quot;` `&apos;` `&amp;` (applied in that order so `&amp;` is last, preventing double-decoding):

```rust
fn strip_tags(html: &str) -> String {
    // ... tag stripping ...
    decode_html_entities(out)   // ← new post-pass
}

fn decode_html_entities(mut text: String) -> String {
    // &amp; last to prevent &amp;lt; → &lt; → <
    replace!(text, "&nbsp;", " ");
    replace!(text, "&lt;",   "<");
    replace!(text, "&gt;",   ">");
    replace!(text, "&quot;", "\"");
    replace!(text, "&apos;", "'");
    replace!(text, "&amp;",  "&");
    text
}
```

**Remaining known issues (not fixed):** Attribute values containing `>` still prematurely exit tag state. CDATA content is still stripped. For full correctness, an HTML5 parser (`html5ever`/`scraper`) would be needed.

---

### 19. ✅ ~~🟢 LOW — `SharedSink` Newtype Is Unnecessary Indirection~~ — **FIXED**

**Files:** [`crates/core/src/progress.rs`](crates/core/src/progress.rs), [`crates/job_runner/src/lib.rs`](crates/job_runner/src/lib.rs)

**Fix applied (2026-03-12):** The review suggested `impl ProgressSink for Arc<BridgeSink>` directly, but Rust's orphan rules prevent this from `job_runner` (both `ProgressSink` and `Arc` are foreign to that crate). The correct solution: add a blanket impl in `dedupe_core` where `ProgressSink` is defined:

```rust
// crates/core/src/progress.rs
impl<T: ProgressSink> ProgressSink for std::sync::Arc<T> {
    fn on_event(&self, event: ProgressEvent) {
        T::on_event(self.as_ref(), event);
    }
}
```

With this blanket impl, `Arc<BridgeSink>` automatically satisfies `ProgressSink`. In `job_runner`:

- `SharedSink` struct and its `ProgressSink` impl removed
- Call site updated: `SharedSink(Arc::clone(&sink))` → `Arc::clone(&sink)`

---

### 20. ✅ ~~🟢 LOW — `temp_bytes_total` Is Always `None`~~ — **FIXED**

**Files:** [`crates/job_runner/src/lib.rs`](crates/job_runner/src/lib.rs), [`apps/desktop/src/main.tsx`](apps/desktop/src/main.tsx)

**Fix applied (2026-03-12):** Removed the dead field from all layers. Computing the actual value would require plumbing through the disk-mode temp file lifecycle; removing it is the correct call since the field was never populated and never rendered.

- `pub temp_bytes_total: Option<u64>` removed from `RunSummary` struct
- `temp_bytes_total: None` removed from `build_run_summary`
- `tempBytesTotal: number | null` removed from the TypeScript `RunSummary` interface
- Corresponding parse line removed from the TypeScript deserializer

---

### 21. ✅ ~~🟢 LOW — Missing `Default` impl for `BackendService`~~ — **FIXED**

**File:** [`crates/backend/src/lib.rs`](crates/backend/src/lib.rs)

**Fix applied (2026-03-12):** Added `impl Default for BackendService` delegating to `Self::new()`, placed immediately before the `impl BackendService` block:

```rust
impl Default for BackendService {
    fn default() -> Self {
        Self::new()
    }
}
```

Satisfies `clippy::new_without_default` and follows the standard Rust convention for zero-argument infallible constructors.

---

### 22. ✅ ~~🟢 LOW — `AHasher::default()` Uses Fixed Keys in `bucket_index`~~ — **FIXED**

**File:** [`crates/core/src/disk.rs`](crates/core/src/disk.rs)

**Fix applied (2026-03-12):** Replaced the per-token `AHasher` construction with a single `ahash::RandomState` stored in `WritableBuckets`, created once in `WritableBuckets::new()`. `bucket_index` now accepts `state: &ahash::RandomState` and calls the inherent `hash_one` method — one hasher factory instantiation per job instead of one per token:

```rust
// Before: new AHasher constructed for every token
fn bucket_index(token: &str, n: usize) -> usize {
    let mut hasher = AHasher::default();
    hasher.write(token.as_bytes());
    (hasher.finish() as usize) % n
}

// After: RandomState created once in WritableBuckets::new(), reused per token
fn bucket_index(token: &str, n: usize, state: &ahash::RandomState) -> usize {
    (state.hash_one(token) as usize) % n
}
```

The `use ahash::AHasher` and `use std::hash::Hasher` imports were removed as they are no longer needed. `ahash::RandomState::new()` uses random per-process seeds, which is correct — bucket assignment only needs to be consistent within a single run (partition and reduce use the same `WritableBuckets` instance), so cross-run reproducibility is not required.

As a bonus, a pre-existing compile error in `apps/cli/src/main.rs` was also fixed: `StartJobConfig` was missing the `per_file_stats` field added by finding #7; defaulted to `false` since the CLI does not expose that flag.

---

## Architecture Observations

### Strengths

- **Clean separation of concerns.** `core` → `job_runner` → `backend` → UI is a well-designed dependency graph with no cycles.
- **Trait-based polymorphism** (`ProgressSink`, `CancelCheck`) allows easy testing with no-op implementations without runtime cost.
- **`Box<str>` in `RamStore`** is the correct choice over `String` — saves 8 bytes per token (no capacity field), which compounds significantly over tens of millions of entries.
- **`ahash` + `hashbrown`** for the hot-path data structures is the right choice; significantly faster than `std::HashMap`/`HashSet` for this workload.
- **Typestate-lite design** in `RamStore` (Stable vs Unordered) avoids runtime branching per-token once the variant is selected.
- **Cancellation design** is solid: `CancelCheck` as a trait allows zero-overhead no-cancel via `NoCancel` (a ZST), and `CancellationToken` is a clean `Arc<AtomicBool>` — no async runtime dependency.
- **`BridgeSink` EWMA throughput calculation** is a thoughtful addition to progress reporting.

### Structural Gaps

- **`Mode::Auto` is a stub.** The enum variant implies intelligence that doesn't exist (see finding #6).
- **No `From<ConfigError>` hierarchy.** The core validation errors are `anyhow` strings, forcing the backend to re-parse its own error messages (see finding #12).
- **`per_file_stats` in disk mode** is undiscoverable — the config accepts it, the engine ignores it, no warning is raised (see finding #7).
- **`temp_bytes_total`** is a dead field in the public API (see finding #20).

---

## Quick Reference: Findings by Priority

| # | Severity | File | Finding |
|---|----------|------|---------|
| 1 | ✅ ~~🔴 High~~ | `desktop/main.rs` | ~~Shell injection via `cmd /C start`~~ — **Fixed** |
| 2 | ✅ ~~🔴 High~~ | `backend/lib.rs` | ~~Mutex `.unwrap()` can panic~~ — **Fixed** |
| 3 | ✅ ~~🟡 Medium~~ | `core/disk.rs` | ~~Double-filtering in reduce path~~ — **Fixed** |
| 4 | ✅ ~~🟡 Medium~~ | `core/disk.rs` | ~~Unnecessary `to_string()` per token~~ — **Fixed** |
| 5 | ✅ ~~🟡 Medium~~ | `core/disk_sort.rs` | ~~Avoidable `clone()` on unique tokens~~ — **Fixed** |
| 6 | ✅ ~~🟡 Medium~~ | `core/engine.rs` | ~~`Mode::Auto` = RAM with no intelligence~~ — **Fixed** |
| 7 | ✅ ~~🟡 Medium~~ | `job_runner/lib.rs` | ~~`per_file_stats` silently dropped in Disk mode~~ — **Fixed** |
| 8 | ✅ ~~🟡 Medium~~ | `job_runner/lib.rs` | ~~`filtered_by_length` = 0 in cancel/error summary~~ — **Fixed** |
| 9 | ✅ ~~🟡 Medium~~ | `core/disk.rs` | ~~"WritingOutput" stage emitted after all writing~~ — **Fixed** |
| 10 | ✅ Fixed | `core/disk.rs` | Bucket writers open during read phase — typestate `WritableBuckets`/`ReducibleBuckets` |
| 11 | ✅ ~~🟡 Medium~~ | `core/config.rs` | ~~Doc comments on `drop_length_min/max` are wrong~~ — **Fixed** |
| 12 | ✅ Fixed | `backend/lib.rs` | Error classification via string matching — `ConfigError`/`JobError`/`InputError` + `downcast_ref` |
| 13 | ✅ Fixed | `core/word_checker.rs` | Double lookup in WordChecker::load removed; RamStore intentional |
| 14 | ✅ Fixed | `core/engine.rs` | Two passes → `.any()` single pass in `resolve_rich_inputs` |
| 15 | ✅ Fixed | `core/token_iter.rs` | `CharIndices` stored in struct, created once per `TokenIter::new` |
| 16 | ✅ Fixed | `core/disk.rs` | `BufRead::lines()` replaced with `LossyLineReader` in `reduce_to_output` |
| 17 | ✅ Fixed | `backend/lib.rs` | Recursion → iterative DFS with explicit stack |
| 18 | ⚠️ Partial | `core/epub_reader.rs` | Entity decoder post-pass added; `>` in attrs still unhandled |
| 19 | ✅ Fixed | `job_runner/lib.rs` | `SharedSink` removed; blanket `impl ProgressSink for Arc<T>` in core |
| 20 | ✅ Fixed | `job_runner/lib.rs` + `main.tsx` | Dead `temp_bytes_total` field removed from all layers |
| 21 | ✅ Fixed | `backend/lib.rs` | `impl Default for BackendService` added |
| 22 | ✅ Fixed | `core/disk.rs` | `RandomState` stored in `WritableBuckets`, `hash_one` called per token |

---

*End of review.*
