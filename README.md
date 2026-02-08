# Dupli-Annihilator-G - Documentation Baseline

**Principal Author:** Giuseppe Rojas

This repository contains the final, structured product documentation, organized by domain (product, engine, UI, and delivery plan), with no source code included.

## Documentation Baseline Metadata
- Baseline version: `V1.0`
- Current document-set version: `V1.1.2`
- Baseline date: `2026-02-08`
- Governance model: controlled documents with traceable decision workflow
- Control reference: `docs/06_DOCUMENT_CONTROL.md`

## Recommended Reading Order
1. `docs/00_FINAL_DOCUMENTATION_INDEX.md`
2. `docs/01_FINAL_EXECUTIVE_SUMMARY.md`
3. `docs/02_FINAL_ENGINE_SPECIFICATION.md`
4. `docs/03_FINAL_UI_TAURI_SPECIFICATION.md`
5. `docs/04_FINAL_PM_IMPLEMENTATION_PLAN.md`
6. `docs/05_PENDING_DECISIONS.md`
7. `docs/06_DOCUMENT_CONTROL.md`

## Current Status
- Documentation is split into separate, linked documents.
- Core functional definitions are closed for V1.
- V1 decisions are recorded and traceable.

## Closed V1 Decisions
1. Default output separator: `"\n"`.
2. `Mode=Auto`: explicit alias of `Ram` in V1.
3. `PreserveFirstSeen` in DISK mode: allowed, with a strong UI warning.
4. V1 UI languages: English (`en`) and Simplified Chinese (`zh-CN`).
5. Scalable i18n: no hardcoded UI text; key-based localization only.
6. V1 SLOs: smooth UI updates (4-10 Hz), approximate ETA when reliable, mode-specific memory control, and dataset-based performance baselining.

## Documentation Maintenance Rule
If a product decision changes, update `docs/05_PENDING_DECISIONS.md` first, then sync:
- `docs/01_FINAL_EXECUTIVE_SUMMARY.md`
- `docs/02_FINAL_ENGINE_SPECIFICATION.md`
- `docs/03_FINAL_UI_TAURI_SPECIFICATION.md`
- `docs/04_FINAL_PM_IMPLEMENTATION_PLAN.md`

## Approval and Change Governance
- All documentation changes must follow `docs/06_DOCUMENT_CONTROL.md`.
- Decision-impacting edits require decision-log update plus cross-document synchronization.
- No contract-level change is considered valid without governance traceability.

