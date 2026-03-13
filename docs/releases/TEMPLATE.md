# Dupli-Annihilator-G Release Notes

## Summary
- Professional desktop deduplication workflow for very large text datasets.
- Native installers for Windows, macOS, and Linux.
- High-performance Rust engine with RAM and DISK execution strategies.

## Highlights
- Multi-file deduplication with deterministic output controls.
- Mission Report with exportable run diagnostics.
- In-app update checks and release automation pipeline.
- Multilingual UI support.

## Core Capabilities
- Input handling:
  - File picker and drag-and-drop.
  - Multiple input files merged into one output stream.
- Processing:
  - Modes: `AUTO`, `RAM`, `DISK`.
  - Ordering: `preserve_first_seen`, `alphabetical`, `unordered_fast`.
  - Optional normalization: `trim`, `drop_empty`.
  - Custom output separator (escaped or raw).
- Observability:
  - Live stage/progress/throughput/ETA telemetry.
  - Final Mission Report with reduction metrics, timeline, and warnings.
  - Open output/folder, copy report, export JSON, run again.
- Operations:
  - Release version/tag coherence checks.
  - Tag-from-main enforcement in CI.
  - Automated cross-platform release publishing.

## Notes
- No breaking changes expected for standard desktop usage.
- Windows first installs should use `-setup.exe`; macOS first installs should use `.dmg`.
- Patch/minor updates can flow through the in-app updater when the signed updater lane is healthy; major updates remain manual.
- Refer to `README.md` and `docs/` for architecture and operational details.
