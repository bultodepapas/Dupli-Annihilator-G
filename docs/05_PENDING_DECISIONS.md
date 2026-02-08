# V1 Decision Log (Status Tracker)

## Related Documents
- `README.md`
- `docs/00_FINAL_DOCUMENTATION_INDEX.md`
- `docs/01_FINAL_EXECUTIVE_SUMMARY.md`
- `docs/02_FINAL_ENGINE_SPECIFICATION.md`
- `docs/03_FINAL_UI_TAURI_SPECIFICATION.md`
- `docs/04_FINAL_PM_IMPLEMENTATION_PLAN.md`

## Objective
Maintain a traceable, authoritative log of V1 product decisions and ensure alignment across functional, technical, and delivery documentation.

## D-01 Default Output Separator
- Status: `CLOSED`
- Final decision:
  `"\n"` is the default output separator.
- Impact:
  Dataset-oriented readability, simpler QA validation, and lower ambiguity in examples.

## D-02 `Mode=Auto` in V1
- Status: `CLOSED`
- Final decision:
  `Auto` behaves as an explicit alias of `Ram` in V1.
- UI requirement:
  Mandatory tooltip stating that true heuristic auto-selection is deferred.
- Impact:
  Avoids early over-engineering and prevents non-deterministic behavior expectations.

## D-03 `PreserveFirstSeen` Policy in DISK Mode
- Status: `CLOSED`
- Final decision:
  Keep selectable in DISK mode with strong warning + visible tooltip.
- Minimum message:
  Global first-seen order is not guaranteed in DISK mode for V1.
- Impact:
  Preserves transparency without blocking advanced usage.

## D-04 V1 UI Languages
- Status: `CLOSED`
- Final decision:
  V1 supports English (`en`) and Simplified Chinese (`zh-CN`).
- Architecture requirement:
  Key-based i18n with no hardcoded component text.
- Scalability requirement:
  Localization structure must allow adding future languages without major refactor.
- Impact:
  International-ready baseline in V1 with controlled localization debt.

## D-05 V1 Performance SLOs for Acceptance
- Status: `CLOSED`
- Final decision:
  Adopt initial V1 SLOs for controlled QA environments.
- V1 SLO baseline:
  1. UI: no perceived freeze during execution; progress updates between 4 and 10Hz.
  2. ETA: show approximate ETA only when reliability is sufficient; otherwise show `-`.
  3. RAM mode: target peak memory <= 75% of free RAM at job start.
  4. DISK mode: bounded and stable memory profile with disk spill prioritization.
  5. Performance baseline: record throughput and elapsed time by reference dataset to detect regressions per release.
- Note:
  Fine-grained numeric targets by dataset size are calibrated in the first benchmark cycle.

## Maintenance Policy
If a closed decision changes:
1. Update this entry with reason and date.
2. Sync `docs/01_FINAL_EXECUTIVE_SUMMARY.md`.
3. Sync `docs/02_FINAL_ENGINE_SPECIFICATION.md` and/or `docs/03_FINAL_UI_TAURI_SPECIFICATION.md`.
4. Sync delivery impact in `docs/04_FINAL_PM_IMPLEMENTATION_PLAN.md`.

