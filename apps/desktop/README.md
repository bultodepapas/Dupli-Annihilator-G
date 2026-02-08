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
- `next_events` (batched polling)

## Run in development (Windows)
1. From repository root, install frontend dependencies:
   - `npm --prefix apps/desktop install`
2. Start frontend dev server:
   - `npm --prefix apps/desktop run dev`
3. In a second terminal, run desktop app:
   - `cargo run --manifest-path apps/desktop/src-tauri/Cargo.toml`

## Build frontend bundle
- `npm --prefix apps/desktop run build`
