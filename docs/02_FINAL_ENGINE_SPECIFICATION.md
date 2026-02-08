# Rust Engine Specification (Final, No Code)

## Related Documents
- `README.md`
- `docs/00_FINAL_DOCUMENTATION_INDEX.md`
- `docs/01_FINAL_EXECUTIVE_SUMMARY.md`
- `docs/03_FINAL_UI_TAURI_SPECIFICATION.md`
- `docs/04_FINAL_PM_IMPLEMENTATION_PLAN.md`
- `docs/05_PENDING_DECISIONS.md`

## 1) Objective
Process one or more text files, extract tokens, remove duplicates, and export a single output file with a configurable separator, while maintaining high performance at large scale.

## 2) Functional Contract

### 2.1 Input
- Type: plain text files (for example `.txt`, simple `.csv`, and equivalent text formats).
- Multiple input files are supported.

### 2.2 Tokenization Rules
- Active input delimiters by default:
  - whitespace (spaces, tabs, line breaks),
  - comma `,`,
  - semicolon `;`.
- The engine extracts tokens only.

### 2.3 Normalization
- Per-token `trim`: ON by default.
- Empty-token drop: ON by default.
- No lowercase normalization and no case folding.

### 2.4 Deduplication
- Exact, case-sensitive, and Unicode-safe.
- Guarantee: no probabilistic or approximate techniques.

### 2.5 Output
- Unique tokens joined by `output_separator` as an arbitrary string.
- Supports simple and compound separators (for example `","`, `", "`, `",\n"`, `"\n"`, `";\n"`, `"\f"`).
- No trailing separator.
- No extra line breaks outside the configured separator.

## 3) Operating Modes

### 3.1 Mode
- `Ram`: for datasets that fit in memory.
- `Disk`: for large datasets using temporary disk storage.
- `Auto`: in V1, behaves as an alias of `Ram` (true heuristic mode is deferred).

### 3.2 Ordering
- `PreserveFirstSeen`: preserves first-seen order (stable in RAM).
- `Alphabetical`: deterministic lexicographic order by UTF-8 bytes.
- `UnorderedFast`: maximum speed without order guarantee.

### 3.3 Submode for `Disk + Alphabetical`
- `FastBucketLocal`:
  - default recommendation,
  - very fast,
  - does not guarantee globally perfect A-Z ordering.
- `GlobalPerfect`:
  - external merge sort,
  - guarantees globally perfect A-Z ordering,
  - higher I/O and CPU cost.

## 4) Final Guarantees Matrix

### 4.1 RAM
| Ordering | Exact Dedupe | Output Order | Performance |
| --- | --- | --- | --- |
| PreserveFirstSeen | Yes | Global stable first-seen order | Very High |
| Alphabetical | Yes | Globally perfect A-Z | High |
| UnorderedFast | Yes | Not guaranteed | Maximum |

### 4.2 DISK
| Ordering | Variant | Exact Dedupe | Output Order | Performance |
| --- | --- | --- | --- | --- |
| UnorderedFast | N/A | Yes | Not guaranteed | Very High |
| Alphabetical | FastBucketLocal | Yes | Bucket-local ordering, not globally perfect | Very High |
| Alphabetical | GlobalPerfect | Yes | Globally perfect A-Z | Medium/High |
| PreserveFirstSeen | N/A | Yes | Not globally guaranteed in DISK mode | High |

## 5) Final Architecture (High Level)
- Workspace-based repository.
- `crates/core` as reusable and testable engine.
- Expected integrations:
  - Tauri desktop UI,
  - optional CLI,
  - tests and benchmarks.

### 5.1 Modular Responsibilities (No Implementation)
- Config: execution contract.
- Tokenization: streaming parser with fixed delimiters.
- RAM dedupe: fast execution with ordering-dependent behavior.
- DISK dedupe: hash buckets and/or global order through external merge sort.
- Writer: streaming output with configured separator.
- Progress/Stats: aggregated telemetry for UI consumption.
- Engine: end-to-end job orchestration.

## 6) Confirmed Performance Principles
- Streaming reads and writes.
- Minimize allocations in hot paths.
- Use high-performance hash structures for dedupe.
- Keep heavy processing out of the frontend.
- Avoid per-token UI event granularity.

## 7) Required Observability
- Best-effort global progress.
- Current stage.
- Operational counters:
  - seen tokens,
  - unique tokens,
  - duplicates,
  - throughput,
  - elapsed time,
  - approximate ETA when reliable.

## 8) Limits and Correctness Notes
1. Alphabetical order is deterministic by UTF-8 bytes, not locale-aware human collation.
2. Unicode-aware `trim` prioritizes correctness.
3. Global `PreserveFirstSeen` in DISK mode is not guaranteed in V1.
4. `GlobalPerfect` performance depends on storage throughput and run sizing.

## 9) Recommended Operational Defaults
- `mode = Ram`
- `mode_auto_behavior_v1 = RamAlias`
- `ordering = PreserveFirstSeen`
- `disk_alphabetical_mode = FastBucketLocal` when applicable
- `disk_buckets = 256`
- `disk_run_bytes = 256MB` (adjustable to 512MB according to hardware)
- `trim = true`
- `drop_empty = true`
- `output_separator_default = "\n"`

## 10) Planned Evolution (Without Breaking V1)
1. True heuristic `Auto` mode based on sampling.
2. Merge-path allocation optimization.
3. Multi-pass merge for extreme run counts.
4. ETA estimation refinement in large DISK workflows.

