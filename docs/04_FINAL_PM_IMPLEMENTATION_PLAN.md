# Final PM Implementation Plan (V1)

## Related Documents
- `README.md`
- `docs/00_FINAL_DOCUMENTATION_INDEX.md`
- `docs/01_FINAL_EXECUTIVE_SUMMARY.md`
- `docs/02_FINAL_ENGINE_SPECIFICATION.md`
- `docs/03_FINAL_UI_TAURI_SPECIFICATION.md`
- `docs/05_PENDING_DECISIONS.md`

## 1) Plan Objective
Provide an execution-grade delivery plan for a senior team to ship V1 with controlled scope, measurable quality, and low rework risk.

## 2) Delivery Principles
- Build from correctness to scale: correctness first, throughput second, polish third.
- Freeze contracts early: engine config, stage taxonomy, and IPC payload shape.
- Enforce evidence-based gating: every phase must exit on measurable criteria.
- Keep decision traceability: all scope changes mapped to decision log updates.

## 3) Scope Baseline (V1)
In scope:
- Rust core with RAM/DISK execution and ordering variants.
- Tauri single-screen UX with progress/telemetry.
- i18n for `en` and `zh-CN`.
- V1 SLO baseline and acceptance QA.

Out of scope:
- heuristic Auto mode,
- locale-aware collation,
- global first-seen order guarantee in DISK mode,
- token-level UI log streaming.

## 4) Workstreams

### 4.1 Core Engine Workstream
- finalize config validation behavior,
- complete and verify mode/order pipelines,
- expose stable stats/progress surface,
- run benchmark baseline pack.

### 4.2 Tauri Backend Workstream
- implement command handlers and job lifecycle,
- implement cancellation handshake and cleanup,
- standardize event payloads,
- enforce background execution model.

### 4.3 Frontend Workstream
- implement single-screen layout and control model,
- implement telemetry rendering under throttled updates,
- implement validation and runtime state transitions,
- implement i18n key pipeline for `en` and `zh-CN`.

### 4.4 QA and Reliability Workstream
- functional acceptance suite,
- regression checks for ordering guarantees,
- stress checks for UI responsiveness,
- localization and accessibility verification.

### 4.5 Release and Operations Workstream
- packaging and versioning process,
- release notes with known limits,
- rollback-ready artifact handling.

## 5) Phase Plan and Exit Gates

### Phase 0 - Documentation Baseline
Deliverables:
- approved final engine spec,
- approved final UI/Tauri spec,
- closed decision log,
- acceptance matrix approved.

Exit gate:
- stakeholders sign off scope and non-goals.

### Phase 1 - Core Engine Integration Readiness
Deliverables:
- engine functional parity with spec,
- config validation enforcement,
- initial benchmark profile,
- correctness checks for mode/order matrix.

Exit gate:
- no unresolved correctness blockers in engine behavior.

### Phase 2 - Tauri Shell + Runtime Integration
Deliverables:
- commands/events wired,
- deterministic run/cancel lifecycle,
- telemetry path available end-to-end,
- i18n infrastructure active.

Exit gate:
- end-to-end run and cancel flows pass in target environments.

### Phase 3 - UX Performance and Hardening
Deliverables:
- stable UI under target event rates,
- ETA presentation policy enforced,
- warning copy and edge-state UX finalized,
- accessibility and localization pass.

Exit gate:
- performance and UX criteria meet baseline acceptance.

### Phase 4 - Final QA and Release Readiness
Deliverables:
- acceptance matrix pass,
- security/capability review pass,
- release candidate package + notes.

Exit gate:
- release board approval.

## 6) Milestones and Artifacts
- M1: Contract freeze package (spec + decision log + payload schema).
- M2: Engine verification package (tests + benchmark baseline).
- M3: Integrated app package (run/cancel + telemetry + i18n).
- M4: QA evidence package (acceptance checklist + issue closure).
- M5: Release candidate package (signed artifacts + known limits).

## 7) RACI-Style Role Model
- Product owner: scope, tradeoff, and final acceptance authority.
- Tech lead (Rust): engine and backend contract ownership.
- Frontend lead: UX behavior, rendering performance, i18n enforcement.
- QA lead: acceptance matrix ownership and defect gatekeeping.
- Release manager: artifact integrity, release gating, rollback plan.

## 8) Dependency Model
- Frontend depends on IPC contract freeze.
- ETA UI depends on stable telemetry metrics.
- QA automation depends on deterministic stage model and terminal states.
- Release packaging depends on completion of security/capability review.

## 9) Risk Register

### Risk A - Event Flood Degrades UI
Trigger:
- visible frame drops during processing.
Mitigation:
- backend event cap at 4-10Hz,
- frontend batching/throttling,
- no token-level UI streams.

### Risk B - Ordering Guarantees Misunderstood
Trigger:
- user confusion around DISK `PreserveFirstSeen` behavior.
Mitigation:
- warning copy in UI,
- guarantees matrix mirrored in docs.

### Risk C - ETA Instability
Trigger:
- ETA oscillates and loses trust.
Mitigation:
- approximate ETA policy,
- display `-` when confidence is low.

### Risk D - GlobalPerfect Throughput Bottleneck
Trigger:
- unacceptable latency on slower storage.
Mitigation:
- keep `FastBucketLocal` as default,
- expose `GlobalPerfect` as precision mode with warning.

### Risk E - Late Contract Changes Cause Rework
Trigger:
- payload/schema updates after frontend implementation.
Mitigation:
- contract freeze milestone and formal change-control workflow.

## 10) V1 SLOs and Operational Metrics
- UI responsiveness: no perceived freeze; progress update cadence 4-10Hz.
- ETA: shown only when reliability threshold is met.
- RAM mode memory: target peak <= 75% of free RAM at job start.
- DISK mode memory: bounded profile with disk spill prioritization.
- Baseline performance: throughput and elapsed tracked by reference dataset.

## 11) Quality Gates

### Gate 1 - Functional Correctness
- mode/order matrix behavior matches specification.
- separator semantics and normalization behavior verified.

### Gate 2 - Integration Reliability
- command/event contract stable and backward-compatible for V1.
- run/cancel lifecycle is deterministic.

### Gate 3 - UX and Non-Functional Quality
- UI responsiveness under load passes.
- accessibility checks pass (contrast + keyboard + reduced motion).
- localization checks pass (`en`, `zh-CN`).

### Gate 4 - Release Readiness
- known limits documented,
- high/critical defects resolved or formally accepted,
- release package signed and reproducible.

## 12) Reporting Cadence
- Daily engineering sync: blockers and critical path updates.
- Twice-weekly quality review: defect trends and acceptance progress.
- Weekly stakeholder review: scope health, risks, and release confidence.

## 13) Change Governance
Any change affecting parsing, ordering guarantees, IPC shape, or SLO baselines must include:
1. decision-log update in `docs/05_PENDING_DECISIONS.md`,
2. impact statement (technical, UX, QA, release),
3. approval record from product and tech leads,
4. synchronized updates in related specs.

## 14) Release Definition of Done
1. All MUST requirements in UI and engine specs are met.
2. Acceptance matrix is fully passed.
3. No critical blockers remain in output correctness or cancellation flow.
4. Performance and SLO baselines are met in controlled environment.
5. Documentation set is synchronized and traceable.

## 15) Post-Release Follow-Up
- compare production telemetry with benchmark baseline,
- triage feature requests against non-goal list,
- schedule V1.x roadmap items (heuristic Auto, advanced telemetry, localization expansion).
