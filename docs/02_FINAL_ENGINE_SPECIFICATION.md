# Final Engine Specification (Rust Core, No Code)

## Related Documents
- `README.md`
- `docs/00_FINAL_DOCUMENTATION_INDEX.md`
- `docs/01_FINAL_EXECUTIVE_SUMMARY.md`
- `docs/03_FINAL_UI_TAURI_SPECIFICATION.md`
- `docs/04_FINAL_PM_IMPLEMENTATION_PLAN.md`
- `docs/05_PENDING_DECISIONS.md`
- `docs/06_DOCUMENT_CONTROL.md`

## 1) Scope and Intent
This document defines the final V1 engine behavior derived from the final engine section of `ideaV0.01.md`.

It is intentionally implementation-ready and code-free. It focuses on:
- exact behavioral guarantees,
- mode-by-mode processing semantics,
- performance and correctness constraints,
- operational defaults and acceptance expectations.

## 2) Problem Statement
Given one or more plain-text input files, the engine must:
1. extract tokens using fixed delimiters,
2. optionally normalize tokens,
3. remove duplicates exactly (case-sensitive),
4. emit one output file with user-defined separator semantics,
5. support both memory-fit and very-large datasets.

## 3) Core Terminology
- Token: contiguous text segment not containing configured input delimiters.
- Input delimiters (V1 fixed): whitespace, comma `,`, semicolon `;`.
- Unique token: token not previously accepted under exact byte equality.
- Ordering: output arrangement strategy (`PreserveFirstSeen`, `Alphabetical`, `UnorderedFast`).
- Bucket: hash-partitioned temp file shard used in DISK mode.
- Run: sorted deduplicated temp segment used by external merge sort.

## 4) Functional Contract

### 4.1 Input Contract
- Supported input: plain-text files (`.txt`, simple `.csv`, equivalent text files).
- Multiple files are processed as one logical stream (in configured file order).
- Input sources are treated as uncontrolled and heterogeneous.

### 4.2 Tokenization Contract
- Delimiters are fixed in V1:
  - `char::is_whitespace()` domain,
  - `,`,
  - `;`.
- Tokenization is Unicode-safe and operates on valid UTF-8 character boundaries.

### 4.3 Normalization Contract
- `trim`: optional, default ON.
- `drop_empty`: optional, default ON.
- No lowercase conversion, no Unicode case folding.

### 4.4 Deduplication Contract
- Exact dedupe only (no probabilistic approximation).
- Case-sensitive by design.
- Unicode-safe comparison through exact string identity.

### 4.5 Output Contract
- Output is a sequence of unique tokens joined by `output_separator`.
- `output_separator` is an arbitrary string (already interpreted before reaching core).
- No trailing separator is written.
- No extra line breaks are inserted beyond separator content.

## 5) Configuration Contract (V1)

### 5.1 Required Fields
- `inputs`: non-empty list of file paths.
- `output`: output file path.
- `output_separator`: non-empty final separator string.
- `mode`: `Auto | Ram | Disk`.
- `ordering`: `PreserveFirstSeen | Alphabetical | UnorderedFast`.
- `trim`: boolean.
- `drop_empty`: boolean.
- `disk_buckets`: DISK partition count.
- `disk_alphabetical_mode`: `FastBucketLocal | GlobalPerfect`.
- `disk_run_bytes`: target bytes per run in external sort.

### 5.2 Validation Rules
- `inputs` must not be empty.
- `output_separator` must not be empty.
- If `mode=Disk`:
  - `disk_buckets >= 8`.
  - `disk_run_bytes >= 1_000_000` bytes.

### 5.3 V1 Defaults
- `mode = Ram`
- `mode_auto_behavior_v1 = RamAlias`
- `ordering = PreserveFirstSeen`
- `output_separator_default = "\n"`
- `trim = true`
- `drop_empty = true`
- `disk_buckets = 256`
- `disk_alphabetical_mode = FastBucketLocal`
- `disk_run_bytes = 256MB` (512MB on stronger hardware)

## 6) Operating Modes and Ordering

### 6.1 Mode
- `Ram`: in-memory dedupe path for memory-fit workloads.
- `Disk`: temp-storage-backed path for large datasets.
- `Auto` (V1): explicit alias to `Ram`.

### 6.2 Ordering
- `PreserveFirstSeen`: preserve first accepted appearance order.
- `Alphabetical`: deterministic lexical order by UTF-8 bytes.
- `UnorderedFast`: no ordering guarantee, maximize throughput.

### 6.3 DISK Alphabetical Submodes
- `FastBucketLocal`:
  - hash partition + per-bucket sort,
  - very high throughput,
  - not globally perfect A-Z.
- `GlobalPerfect`:
  - external merge sort (run generation + k-way merge),
  - globally perfect A-Z,
  - higher I/O and CPU cost.

## 7) Execution Pipelines

### 7.1 RAM Pipeline
1. Initialize RAM store based on ordering:
   - stable store for `PreserveFirstSeen`/`Alphabetical`,
   - unordered store for `UnorderedFast`.
2. Stream all input files line-by-line.
3. Tokenize each line with fixed delimiters.
4. Apply normalization (`trim`, `drop_empty`).
5. Insert into store and update counters.
6. Materialize unique token list.
7. If `Alphabetical`, sort once before output.
8. Stream tokens to output writer with separator policy.

### 7.2 DISK Bucket Pipeline (`UnorderedFast`, `PreserveFirstSeen`, `Alphabetical+FastBucketLocal`)
1. Create temporary directory and N bucket files.
2. Partition phase:
   - tokenize and normalize input stream,
   - compute bucket index from token hash,
   - append token to bucket file (internal line format).
3. Reduction phase per bucket:
   - load one bucket,
   - dedupe in RAM,
   - optional local alphabetical sort,
   - append reduced output to final writer.
4. Finalize output and release temp resources.

### 7.3 DISK GlobalPerfect Pipeline (`Alphabetical+GlobalPerfect`)
1. Run generation:
   - stream tokens,
   - accumulate until `disk_run_bytes` target,
   - sort + dedupe run,
   - flush run file.
2. K-way merge:
   - open all run readers,
   - merge smallest token first,
   - global dedupe during merge using last-written tracking,
   - stream to final output writer.
3. Finalize and cleanup temporary run files.

### 7.4 AUTO Pipeline (V1)
- `Auto` routes to `Ram` directly.
- No runtime heuristic in V1.
- API remains stable for future heuristic upgrade.

## 8) Guarantees Matrix

### 8.1 RAM
| Ordering | Exact Dedupe | Output Order Guarantee | Performance Profile |
| --- | --- | --- | --- |
| PreserveFirstSeen | Yes | Global stable first-seen order | Very High |
| Alphabetical | Yes | Globally perfect A-Z (UTF-8 byte lexical) | High |
| UnorderedFast | Yes | Not guaranteed | Maximum |

### 8.2 DISK
| Ordering | Variant | Exact Dedupe | Output Order Guarantee | Performance Profile |
| --- | --- | --- | --- | --- |
| UnorderedFast | N/A | Yes | Not guaranteed | Very High |
| Alphabetical | FastBucketLocal | Yes | Bucket-local alphabetical only | Very High |
| Alphabetical | GlobalPerfect | Yes | Globally perfect A-Z | Medium/High |
| PreserveFirstSeen | N/A | Yes | Not globally guaranteed in V1 | High |

## 9) Progress, Stats, and Observability

### 9.1 Progress Event Model
- File lifecycle:
  - `FileStarted { index, total }`
  - `FileFinished { index, total }`
- Counters:
  - `TokensSeen(u64)`
  - `UniqueTokens(u64)`
  - `Duplicates(u64)`

### 9.2 Emission Cadence
- Counter events are emitted in coarse-grained increments (e.g., every 100k) to reduce overhead.
- Engine should remain agnostic to UI frame rate; UI side applies additional throttling.

### 9.3 Stats Contract
- `files`
- `tokens_seen`
- `unique_tokens`
- `duplicates`
- `elapsed`

## 10) Correctness Notes and Intentional Limits
1. Alphabetical sort is deterministic by UTF-8 byte order, not locale-aware collation.
2. Unicode-aware trimming is retained for correctness.
3. In V1, global `PreserveFirstSeen` is guaranteed only in RAM mode.
4. `FastBucketLocal` is intentionally not globally alphabetical.
5. Separator escape interpretation is expected to be resolved before core invocation.
6. Output writer must never append trailing separator.

## 11) Performance Model and Tuning Guidance

### 11.1 Dominant Costs
- Input/output throughput.
- Tokenization and normalization overhead.
- Hashing and dedupe structure behavior.
- Sorting/merge costs for alphabetical paths.

### 11.2 Practical Tuning
- Prefer `Ram + PreserveFirstSeen` for typical workloads.
- Use `Disk` when memory-fit is uncertain.
- Keep `disk_buckets` high enough (default 256) to reduce bucket skew.
- Increase `disk_run_bytes` (256MB to 512MB) to reduce run count in global sort.
- Use `FastBucketLocal` unless globally perfect alphabetical order is mandatory.

### 11.3 UI Boundary for Performance
- Never stream tokens to UI.
- Send only aggregated progress and stats.

## 12) Benchmarking Policy (Criterion)
V1 benchmark coverage should compare:
- RAM `UnorderedFast`,
- RAM `PreserveFirstSeen`,
- RAM `Alphabetical`,
- DISK `Alphabetical + FastBucketLocal`,
- DISK `Alphabetical + GlobalPerfect`.

Operational recommendation:
- start with 300k to 1M tokens for iterative tuning,
- scale to larger datasets after baseline stability,
- track throughput and elapsed deltas release-over-release.

## 13) Error and Validation Expectations
- Engine must fail fast on invalid config.
- Validation failures must be deterministic and explicit.
- File I/O and temp-storage errors must propagate with enough context for UI reporting.

## 14) Integration Notes for Desktop Clients
- Core remains UI-agnostic and reusable.
- Tauri frontend acts as orchestration layer only.
- Output separator should be pre-parsed by UI when escape syntax is supported.

## 15) Known Non-Goals (V1)
- Locale-aware human collation (language-specific alphabetical rules).
- True heuristic `Auto` mode.
- Global first-seen preservation in DISK mode.
- Engine-side rich progress staging beyond the base event model.

## 16) Planned Evolution (Non-Breaking)
1. True heuristic `Auto` selection (sampling + uniqueness estimation).
2. Allocation reduction in merge path.
3. Multi-pass merge strategy for extreme run counts.
4. Optional core-side escape parser for separators.
5. Extended progress/stage model if required by UI telemetry roadmap.
