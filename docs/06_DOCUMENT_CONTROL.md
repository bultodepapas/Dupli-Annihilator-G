# Document Control and Governance (V1)

## 1) Objective
Define the governance system for the documentation baseline, including ownership, versioning, approvals, revision traceability, and change workflow.

## 2) Control Scope
This control policy applies to:
- `README.md`
- `docs/00_FINAL_DOCUMENTATION_INDEX.md`
- `docs/01_FINAL_EXECUTIVE_SUMMARY.md`
- `docs/02_FINAL_ENGINE_SPECIFICATION.md`
- `docs/03_FINAL_UI_TAURI_SPECIFICATION.md`
- `docs/04_FINAL_PM_IMPLEMENTATION_PLAN.md`
- `docs/05_PENDING_DECISIONS.md`
- `docs/06_DOCUMENT_CONTROL.md`

## 3) Baseline Record
- Documentation baseline version: `V1.0`
- Current controlled version: `V1.1.2`
- Effective baseline date: `2026-02-08`
- Principal author: `Giuseppe Rojas`
- Baseline status: `Approved for V1 execution`

## 4) Document Ownership Model
| Document | Primary Owner | Secondary Owner | Governance Critical |
| --- | --- | --- | --- |
| `README.md` | Product/PM | Tech Lead | Yes |
| `docs/00_FINAL_DOCUMENTATION_INDEX.md` | PMO/Delivery | Product | Yes |
| `docs/01_FINAL_EXECUTIVE_SUMMARY.md` | Product | PMO | Yes |
| `docs/02_FINAL_ENGINE_SPECIFICATION.md` | Rust Tech Lead | Product | Yes |
| `docs/03_FINAL_UI_TAURI_SPECIFICATION.md` | Frontend/Tauri Lead | Product | Yes |
| `docs/04_FINAL_PM_IMPLEMENTATION_PLAN.md` | PMO/Delivery Lead | Product | Yes |
| `docs/05_PENDING_DECISIONS.md` | Product + Tech Lead | PMO | Yes |
| `docs/06_DOCUMENT_CONTROL.md` | PMO/Governance | Product | Yes |

## 5) Versioning Policy
Use semantic document-set versioning:
- `MAJOR`: structural or contractual baseline change.
- `MINOR`: added sections/requirements without breaking prior contracts.
- `PATCH`: clarifications, wording, typo fixes, formatting changes.

Version example:
- `V1.2.3` = major 1, minor 2, patch 3.

## 6) Revision Log (Document Set)
| Version | Date | Type | Summary | Approved By |
| --- | --- | --- | --- | --- |
| V1.0.0 | 2026-02-08 | MAJOR | Initial finalized documentation baseline in English with linked specs. | Product + Tech Leads |
| V1.1.0 | 2026-02-08 | MINOR | Senior-level expansion of engine, UI/Tauri, PM, executive summary, and decision governance. | Product + Tech Leads |
| V1.1.1 | 2026-02-08 | PATCH | Added formal document control, ownership matrix, and governance workflow. | PMO/Delivery |
| V1.1.2 | 2026-02-08 | PATCH | Final logic and cross-link consistency audit; unified version metadata across baseline docs. | PMO/Delivery |

## 7) Approval Workflow
Any change tagged as `MAJOR` or `MINOR` requires:
1. Technical review by owning lead.
2. Product review for behavior/scope impact.
3. QA review for acceptance and SLO implications.
4. Governance entry update in this file and, if applicable, in `docs/05_PENDING_DECISIONS.md`.

`PATCH` changes require owner review and revision-log update.

## 8) Change Classification Rules
- Decision impact (mode/order/contract/SLO): at least `MINOR`.
- IPC payload shape change: `MAJOR` unless fully backward-compatible and optional.
- UI copy-only correction: `PATCH`.
- Non-functional requirement change (security/accessibility/performance criteria): `MINOR`.

## 9) Mandatory Synchronization Rules
When `docs/05_PENDING_DECISIONS.md` changes, update in order:
1. `docs/01_FINAL_EXECUTIVE_SUMMARY.md`
2. `docs/02_FINAL_ENGINE_SPECIFICATION.md`
3. `docs/03_FINAL_UI_TAURI_SPECIFICATION.md`
4. `docs/04_FINAL_PM_IMPLEMENTATION_PLAN.md`
5. `README.md` and `docs/00_FINAL_DOCUMENTATION_INDEX.md` if baseline or scope changed

## 10) Auditability Requirements
Each controlled change must keep:
- change reason,
- impacted documents,
- classification (`MAJOR`/`MINOR`/`PATCH`),
- effective date,
- approver role(s).

## 11) Release Gate Dependency
A release candidate is considered documentation-complete only when:
1. All governance-critical docs are internally consistent.
2. Decision log and specs reflect the same active behavior.
3. Revision log is up to date.
4. No unresolved decision conflict remains.

## 12) Future Governance Enhancements
Post-V1 improvements:
1. Add document checksum/signature process per release candidate.
2. Add owner-specific approval checklists.
3. Add automated consistency checks for cross-document references.
