# V1 Decision Log (Governance Record)

## Related Documents
- `README.md`
- `docs/00_FINAL_DOCUMENTATION_INDEX.md`
- `docs/01_FINAL_EXECUTIVE_SUMMARY.md`
- `docs/02_FINAL_ENGINE_SPECIFICATION.md`
- `docs/03_FINAL_UI_TAURI_SPECIFICATION.md`
- `docs/04_FINAL_PM_IMPLEMENTATION_PLAN.md`

## 1) Purpose
Maintain an auditable and traceable record of V1 product decisions.

This file is the authoritative change-control entry point for decision-level changes affecting engine behavior, UI contract, SLOs, and release scope.

## 2) Record Model (Per Decision)
Each decision entry must preserve:
- `Decision ID`
- `Status` (`CLOSED`, `SUPERSEDED`, `DEFERRED`)
- `Owner`
- `Effective Date`
- `Context`
- `Decision`
- `Rationale`
- `Impact`
- `Rollback Risk`
- `Supersedes` (if any)

## 3) Decision Register

### D-01 Default Output Separator
- Status: `CLOSED`
- Owner: Product + Core Engine Leads
- Effective Date: 2026-02-08
- Context: Output must have deterministic, dataset-friendly default formatting.
- Decision: `"\n"` is the default output separator.
- Rationale: Line-based outputs are easier to inspect, diff, and process in data workflows.
- Impact:
  - improves readability for large result sets,
  - reduces ambiguity in QA acceptance artifacts.
- Rollback Risk: Medium (affects expected output formatting and test fixtures).
- Supersedes: N/A

### D-02 `Mode=Auto` Behavior in V1
- Status: `CLOSED`
- Owner: Product + Core Engine Leads
- Effective Date: 2026-02-08
- Context: True heuristic auto-selection increases implementation risk for V1.
- Decision: `Auto` behaves as explicit alias of `Ram` in V1.
- Rationale: Keeps runtime behavior deterministic while preserving API shape for future heuristic upgrade.
- Impact:
  - lowers initial complexity,
  - requires explicit UI tooltip to prevent false expectations.
- Rollback Risk: Low/Medium (future enhancement path exists without API break).
- Supersedes: N/A

### D-03 `PreserveFirstSeen` Policy in DISK Mode
- Status: `CLOSED`
- Owner: Product + Engine + UX Leads
- Effective Date: 2026-02-08
- Context: Global first-seen ordering in DISK mode is substantially more complex and costly.
- Decision: Keep `PreserveFirstSeen` selectable in DISK mode with strong warning and tooltip.
- Rationale: Preserves user choice while being explicit about guarantee limits.
- Impact:
  - transparent UX,
  - avoids implicit incorrect assumptions.
- Rollback Risk: Medium (removing option later could impact user workflows).
- Supersedes: N/A

### D-04 V1 UI Languages
- Status: `CLOSED`
- Owner: Product + Frontend Lead
- Effective Date: 2026-02-08
- Context: Initial international reach is required without localization debt explosion.
- Decision: Support `en` and `zh-CN` in V1.
- Rationale: Balanced scope between market reach and implementation complexity.
- Impact:
  - i18n framework must be key-based,
  - runtime language switch is required,
  - no hardcoded UI strings allowed.
- Rollback Risk: Medium (partial localization rollback creates UX inconsistency).
- Supersedes: N/A

### D-05 V1 Performance SLO Baseline
- Status: `CLOSED`
- Owner: Product + Engineering + QA Leads
- Effective Date: 2026-02-08
- Context: Release quality requires measurable performance acceptance boundaries.
- Decision: Adopt controlled-environment V1 SLO baseline:
  1. UI update cadence: 4-10Hz.
  2. ETA displayed only when reliability threshold is met; otherwise `-`.
  3. RAM mode peak memory target: <= 75% of free RAM at job start.
  4. DISK mode memory remains bounded with spill-first behavior.
  5. Throughput + elapsed tracked per reference dataset for regression control.
- Rationale: Enables objective release gating and repeatable QA evidence.
- Impact:
  - benchmark and QA pipelines must emit comparable metrics,
  - release readiness depends on SLO evidence.
- Rollback Risk: Medium/High (weakening SLOs reduces quality confidence).
- Supersedes: N/A

## 4) Change-Control Workflow
If any `CLOSED` decision changes:
1. Create a new revision entry with updated fields (`Decision`, `Rationale`, `Impact`, `Effective Date`).
2. Mark prior entry as `SUPERSEDED` and reference successor decision ID.
3. Update impacted specs in this order:
   - `docs/01_FINAL_EXECUTIVE_SUMMARY.md`
   - `docs/02_FINAL_ENGINE_SPECIFICATION.md`
   - `docs/03_FINAL_UI_TAURI_SPECIFICATION.md`
   - `docs/04_FINAL_PM_IMPLEMENTATION_PLAN.md`
4. Record release impact in sprint/release notes.

## 5) Governance Rules
- No undocumented decision changes are allowed.
- No contract-shape changes after integration freeze without governance approval.
- All decision updates must include explicit rationale and rollback risk.

## 6) Pending Decision Placeholder (Post-V1)
Use this section only for decisions intentionally deferred beyond V1, with owner and target milestone.
