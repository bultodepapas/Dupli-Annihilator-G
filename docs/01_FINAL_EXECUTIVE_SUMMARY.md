# Final Executive Summary

## Related Documents
- `README.md`
- `docs/00_FINAL_DOCUMENTATION_INDEX.md`
- `docs/02_FINAL_ENGINE_SPECIFICATION.md`
- `docs/03_FINAL_UI_TAURI_SPECIFICATION.md`
- `docs/04_FINAL_PM_IMPLEMENTATION_PLAN.md`
- `docs/05_PENDING_DECISIONS.md`

## 1) Product Vision
Build a desktop application that merges, deduplicates, and exports tokens from multiple text files, with focus on:
- high performance for large datasets,
- deterministic behavior,
- clear UX for technical users.

## 2) Closed Functional Decisions
1. Deduplication is case-sensitive.
   `Perro`, `perro`, and `PERRO` are distinct tokens.
2. Full Unicode support is required.
   Includes accents, special characters, and emojis.
3. Input parsing must be robust for uncontrolled source files.
   Input delimiters: whitespace, comma, and semicolon.
4. Output uses an arbitrary separator string.
   The engine must respect the separator exactly as provided.
5. No trailing separator at the end of output.
6. DISK mode and output ordering must be user-selectable in the UI.
7. Product default output separator: `"\n"`.
8. `Mode=Auto` in V1 is an alias of `Ram` (with explicit UI tooltip).
9. V1 localization: English (`en`) and Simplified Chinese (`zh-CN`), with no hardcoded UI copy.

## 3) Consolidated Output Behavior
- The engine emits unique tokens joined by the final output separator.
- If the separator contains a newline, output spans multiple lines.
- If the separator does not contain a newline, output is typically one logical line.
- No additional line breaks are inserted beyond the configured separator.

## 4) Final Product Strategy
- Rust engine as reusable core.
- Single-screen desktop UI on Tauri v2.
- Aggregated real-time telemetry, without per-token UI logging.
- Approximate ETA prioritized for utility and low overhead.

## 5) Final Mode and Ordering Matrix
- Ordering:
  - `PreserveFirstSeen` (default)
  - `Alphabetical`
  - `UnorderedFast`
- Mode:
  - `Ram`
  - `Disk`
  - `Auto` (V1 behavior: `Ram` alias)
- For `Disk + Alphabetical`:
  - `FastBucketLocal` (recommended default)
  - `GlobalPerfect` (higher precision, lower speed)

## 6) Explicit Functional Limit
`PreserveFirstSeen` guarantees global first-seen order only in RAM mode. In DISK mode, global first-seen order is not guaranteed in V1.

## 7) Recommended Product Defaults
- `mode = Ram`
- `ordering = PreserveFirstSeen`
- `disk_buckets = 256`
- `disk_run_bytes = 256MB` (scalable to 512MB based on hardware)
- `trim = ON`
- `drop_empty = ON`
- `output_separator_default = "\n"`

## 8) Final Quality Criteria
- Exact deduplication correctness.
- UI responsiveness under heavy load.
- Least-privilege security model in Tauri.
- Minimum AA accessibility for contrast and keyboard navigation.
- Clear error and cancellation behavior for real operation.

