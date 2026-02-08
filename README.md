# Dupli-Annihilator-G - Documentation Baseline

**Principal Author:** Giuseppe Rojas

This repository contains the final, structured product documentation, organized by domain (product, engine, UI, and delivery plan), with no source code included.

## Recommended Reading Order
1. `docs/00_FINAL_DOCUMENTATION_INDEX.md`
2. `docs/01_FINAL_EXECUTIVE_SUMMARY.md`
3. `docs/02_FINAL_ENGINE_SPECIFICATION.md`
4. `docs/03_FINAL_UI_TAURI_SPECIFICATION.md`
5. `docs/04_FINAL_PM_IMPLEMENTATION_PLAN.md`
6. `docs/05_PENDING_DECISIONS.md`

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

