# Dupli-Annihilator-G

**Principal Author:** Giuseppe Rojas

Dupli-Annihilator-G is a desktop tool that removes duplicate tokens from text-like files and exports a clean output file.

## Quick Start (End Users)

If you just want to use the app:
1. Download the installer from this repository's **GitHub Releases** page.
2. Open the desktop app.
3. Add one or more input files (picker or drag and drop).
4. Choose the output file path.
5. Click `RUN` and wait for `DONE`.

## What You Can Do

- Merge multiple input files into one deduplicated output.
- Keep first-seen order, sort alphabetically, or run in fastest unordered mode.
- Choose RAM, DISK, or AUTO execution mode.
- Configure output separator (`\n`, `\t`, custom separator, raw separator).
- Monitor progress, throughput, elapsed time, and ETA in real time.
- Review a final `MISSION REPORT` screen with key results, diagnostics, and timeline.
- Open output/folder, copy report, export JSON summary, and run again with same settings.
- Check for updates from inside the app and download/install when updater metadata is available.
- For major versions (for example `1.x` -> `2.x`), the app enforces manual install from Releases.
- Cancel and retry safely.

## How The Output Works

- Deduplication is exact and case-sensitive (`Perro`, `perro`, `PERRO` are different).
- Token delimiters are fixed to whitespace, comma `,`, and semicolon `;`.
- Output is generated with your selected separator.
- No trailing separator is written at the end of the output file.

## Language Support (Desktop UI)

Current UI locales in the app:
- `en`
- `zh-CN`
- `hi`
- `es`
- `fr`
- `ar`
- `bn`
- `pt`
- `ru`
- `ur`

## Project Layout

- Core engine: `crates/core`
- Job orchestration: `crates/job_runner`
- Backend API: `crates/backend`
- CLI app: `apps/cli`
- Desktop app (Tauri + React): `apps/desktop`
- Product and engineering docs: `docs`

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
cd apps/desktop/src-tauri
cargo tauri dev --ci
```

## Build Installers

Windows (run on Windows):
```bash
npm ci --prefix apps/desktop
cargo install tauri-cli --version "^2.0" --locked
cd apps/desktop/src-tauri
cargo tauri build --ci --no-sign
```
Artifacts are generated under `apps/desktop/src-tauri/target/release/bundle` (for example `.exe` and `.msi`).

macOS (run on macOS):
```bash
npm ci --prefix apps/desktop
cargo install tauri-cli --version "^2.0" --locked
cd apps/desktop/src-tauri
cargo tauri build --ci --no-sign
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

Release publishing:
- Pushing a tag that matches `v*` (example: `v1.3.3`) builds installers and publishes a GitHub Release with attached assets.
- Release tags must point to a commit reachable from `main` (release guard in CI).
- Manual runs are also supported via `workflow_dispatch` with optional `tag` input.
- If `TAURI_SIGNING_PRIVATE_KEY` secrets are configured, the workflow builds signed updater artifacts automatically.
- Updater metadata is configured dynamically during CI by `scripts/release/configure-updater.mjs`.

Updater activation (GitHub):
- Required repository secrets:
  - `TAURI_SIGNING_PRIVATE_KEY`
  - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
  - `TAURI_UPDATER_PUBKEY`
- Optional repository variable:
  - `TAURI_UPDATER_ENDPOINT` (defaults to `.../releases/latest/download/latest.json`)
  - `DUPLI_UPDATE_CHANNEL` (optional app runtime/build channel label, default `stable`)

Dry run updater config locally:
```bash
node scripts/release/configure-updater.mjs --dry-run
```

Release prep helper:
```bash
node scripts/release/bump-version.mjs 1.3.1 --dry-run
node scripts/release/bump-version.mjs 1.3.1
```
This updates desktop/package versions, Tauri versions, crate versions, and release tag example text consistently.

Release prep pipeline helper (single command):
```bash
node scripts/release/prepare-release.mjs 1.3.1 --dry-run
node scripts/release/prepare-release.mjs 1.3.1 --commit --tag
```
Useful flags:
- `--allow-dirty` allows running on non-clean worktrees.
- `--skip-build` skips desktop build.
- `--skip-tests` skips workspace tests.
- `--push` pushes `main` and the release tag (requires `--tag`).

