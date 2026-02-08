# Dupli-Annihilator-G

**Principal Author:** Giuseppe Rojas

Dupli-Annihilator-G is a Rust-based duplicate-line processing system with:
- core engine (`crates/core`)
- job orchestration + backend API (`crates/job_runner`, `crates/backend`)
- CLI app (`apps/cli`)
- desktop app (Tauri + React) (`apps/desktop`)
- product and engineering documentation (`docs`)

## Documentation Baseline
- Baseline version: `V1.0`
- Current document-set version: `V1.1.2`
- Baseline date: `2026-02-08`
- Control reference: `docs/06_DOCUMENT_CONTROL.md`

Recommended reading order:
1. `docs/00_FINAL_DOCUMENTATION_INDEX.md`
2. `docs/01_FINAL_EXECUTIVE_SUMMARY.md`
3. `docs/02_FINAL_ENGINE_SPECIFICATION.md`
4. `docs/03_FINAL_UI_TAURI_SPECIFICATION.md`
5. `docs/04_FINAL_PM_IMPLEMENTATION_PLAN.md`
6. `docs/05_PENDING_DECISIONS.md`
7. `docs/06_DOCUMENT_CONTROL.md`

## Local Development

Requirements:
- Rust stable toolchain
- Node.js 20+
- npm

Run tests:
```bash
cargo test --workspace
```

Run desktop UI in dev mode:
```bash
npm ci --prefix apps/desktop
cargo install tauri-cli --version "^2.0" --locked
cargo tauri dev --manifest-path apps/desktop/src-tauri/Cargo.toml
```

## Build Installers

Windows (run on Windows):
```bash
npm ci --prefix apps/desktop
cargo install tauri-cli --version "^2.0" --locked
cargo tauri build --manifest-path apps/desktop/src-tauri/Cargo.toml
```
Artifacts are generated under `apps/desktop/src-tauri/target/release/bundle` (for example `.exe` and `.msi`).

macOS (run on macOS):
```bash
npm ci --prefix apps/desktop
cargo install tauri-cli --version "^2.0" --locked
cargo tauri build --manifest-path apps/desktop/src-tauri/Cargo.toml
```
Artifacts are generated under `apps/desktop/src-tauri/target/release/bundle` (for example `.dmg`/`.app` bundles).

## CI/CD

GitHub Actions workflows:
- `.github/workflows/ci.yml`
- `.github/workflows/desktop-release.yml`

`desktop-release.yml` builds desktop bundles on:
- `windows-latest`
- `macos-latest`

and uploads platform installers as workflow artifacts.

