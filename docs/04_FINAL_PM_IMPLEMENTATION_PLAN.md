# PM Delivery Plan (Final)

## Related Documents
- `README.md`
- `docs/00_FINAL_DOCUMENTATION_INDEX.md`
- `docs/01_FINAL_EXECUTIVE_SUMMARY.md`
- `docs/02_FINAL_ENGINE_SPECIFICATION.md`
- `docs/03_FINAL_UI_TAURI_SPECIFICATION.md`
- `docs/05_PENDING_DECISIONS.md`

## 1) Plan Objective
Translate final specifications into an execution-ready plan for a senior team, with explicit deliverables, measurable milestones, and controlled technical risk.

## 2) Management Approach
- Incremental delivery by phase.
- Prioritize correctness and performance before UI polish.
- Freeze critical contracts (config, events, stages) to prevent rework.

## 3) Recommended Phases

### Phase 0 - Documentation Baseline
Deliverables:
- Approved final engine specification.
- Approved final UI/Tauri specification.
- Closed QA criteria.
- Closed V1 decision log (`docs/05_PENDING_DECISIONS.md`).

Exit:
- Initial decision log signed off by stakeholders.

### Phase 1 - Core Engine Ready for Integration
Deliverables:
- Functional Rust engine with RAM/DISK modes and defined orderings.
- Aggregated progress and stats telemetry.
- Validation of parsing and output rules.

Exit:
- Baseline functional tests pass.

### Phase 2 - Tauri Shell + Operational UX
Deliverables:
- Complete single-screen UI (Inputs, Processing, Export, Run).
- Commands/events integration with backend.
- Robust run/cancel lifecycle behavior.
- Base i18n in place (`en`, `zh-CN`) with no hardcoded strings.

Exit:
- End-to-end flow operational.

### Phase 3 - UX Performance + Observability
Deliverables:
- Backend/frontend throttling enabled.
- Approximate ETA enabled.
- Stable telemetry under high load.

Exit:
- UI remains responsive in high-volume scenarios.

### Phase 4 - Final QA + Hardening
Deliverables:
- Acceptance test suite executed.
- Minimum AA accessibility review complete.
- Security review complete (least-privilege capabilities).

Exit:
- Release candidate approved.

## 4) Work Priority (Recommended Order)
1. Functional correctness of engine behavior.
2. Stable integration contracts.
3. Operational UX with trustworthy progress feedback.
4. Final optimization and hardening.

## 5) Main Risks and Mitigations

### Risk A: UI degradation from excessive event rate
Mitigation:
- Enforce progress event cap at 4-10Hz.
- Batch frontend state updates.

### Risk B: Misinterpretation of ordering guarantees in DISK mode
Mitigation:
- Explicit warning copy for `PreserveFirstSeen` in DISK mode.
- Keep guarantees matrix visible in product documentation.

### Risk C: ETA instability
Mitigation:
- Use approximate ETA model.
- Display `-` when reliability is insufficient.

### Risk D: GlobalPerfect overhead on slower hardware
Mitigation:
- Keep `FastBucketLocal` as default recommendation.
- Expose `GlobalPerfect` as advanced mode with warning.

### Risk E: Rework from late contract changes
Mitigation:
- Freeze mode/order/event/payload naming before full UI implementation.

## 6) Definition of Done (Release)
1. All MUST functional requirements are met.
2. QA acceptance criteria are approved.
3. No critical blockers in cancel flow or output writing.
4. UI remains stable under target load.
5. Known limits and behavior notes are documented.
6. V1 SLOs are met in controlled environment:
   - progress update rate between 4 and 10Hz,
   - ETA shown only when reliable,
   - RAM mode peak memory controlled (target <= 75% of free RAM at job start),
   - DISK mode behavior stable with disk spill strategy.

## 7) Change Governance
Any change to parsing rules, ordering guarantees, or IPC contracts must include:
- technical impact,
- UX impact,
- QA impact,
- decision entry in product changelog.

## 8) Closure Recommendation
Use this documentation set as the single source of truth for the team, and track future adjustments through versioned annexes instead of rewriting historical decisions without traceability.

