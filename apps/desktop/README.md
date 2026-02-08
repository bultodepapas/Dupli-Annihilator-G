# Desktop App (Tauri + React)

This folder contains the desktop application for `Dupli-Annihilator-G`.

## User Flow (What happens in the app)

1. Add input files (picker or drag and drop).
2. Choose processing mode and ordering.
3. Set output file path and separator.
4. Run the job and watch telemetry.
5. Cancel, run again, or retry when needed.

## Features Available in UI

- Inputs:
  - file picker
  - drag and drop support
- Processing controls:
  - mode: `auto`, `ram`, `disk`
  - ordering: `preserve_first_seen`, `alphabetical`, `unordered_fast`
  - disk settings (`disk_buckets`, `disk_run_bytes`, disk alphabetical mode)
- Output controls:
  - separator input
  - separator presets
  - live separator preview
  - raw separator toggle
  - overwrite policy
- Runtime:
  - status chips (`IDLE`, `RUNNING`, `DONE`, `ERROR`, `CANCELED`)
  - action states (`RUN`, `RUN AGAIN`, `RETRY`, `CANCEL`)
  - progress and metrics (files, tokens, unique, duplicates, throughput, elapsed, ETA)
  - terminal `MISSION REPORT` screen with:
    - key results (`unique`, `duplicates`, `reduction`, output details)
    - diagnostics (`warnings`, stage timeline, mode/order context)
    - actions (`OPEN OUTPUT`, `OPEN FOLDER`, `COPY REPORT`, `EXPORT JSON`, `RUN AGAIN`)

## Localization

Current UI locales:
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

Implementation notes:
- UI text is key-based (`apps/desktop/src/i18n.ts`, `apps/desktop/src/locales/`), not hardcoded.
- Selected locale is persisted in local storage key `dupli.locale`.

## Internal Architecture

- Frontend: React + Vite (`apps/desktop`)
- Desktop shell: Tauri (`apps/desktop/src-tauri`)
- Processing path: `crates/backend` -> `crates/job_runner` -> `crates/core`

## Commands exposed by Tauri backend

- `start_job`
- `cancel_job`
- `get_app_info`
- `get_runtime_state`
- `path_exists`
- `next_events` (batched polling)

## Run in development (Windows)

1. From repository root, install frontend dependencies:
   - `npm --prefix apps/desktop install`
2. Run desktop app in Tauri dev mode:
   - `cd apps/desktop/src-tauri`
   - `cargo tauri dev --ci`

## Build frontend bundle

- `npm --prefix apps/desktop run build`
