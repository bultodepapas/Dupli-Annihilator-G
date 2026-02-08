# Desktop App (Tauri + React)

This folder contains the desktop application for `Dupli-Annihilator-G`:
- frontend: React + Vite (`apps/desktop`)
- backend shell: Tauri (`apps/desktop/src-tauri`)
- processing core: `crates/backend` -> `crates/job_runner` -> `crates/core`

## Commands exposed by Tauri backend
- `start_job`
- `cancel_job`
- `get_app_info`
- `get_runtime_state`
- `path_exists`
- `next_events` (batched polling)

## Localization
- Supported UI locales in V1:
  - `en`
  - `zh-CN`
- UI text is key-based (`apps/desktop/src/i18n.ts`, `apps/desktop/src/locales/`), not hardcoded.
- Selected locale is persisted in local storage key `dupli.locale`.

## Run in development (Windows)
1. From repository root, install frontend dependencies:
   - `npm --prefix apps/desktop install`
2. Run desktop app in Tauri dev mode:
   - `cargo tauri dev --manifest-path apps/desktop/src-tauri/Cargo.toml`

## Build frontend bundle
- `npm --prefix apps/desktop run build`
