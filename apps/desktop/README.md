# Desktop Shell (Tauri)

This folder contains the initial Tauri backend shell for `Dupli-Annihilator-G`.

Current status:
- Rust command bridge implemented in `apps/desktop/src-tauri/src/main.rs`.
- Commands exposed:
  - `start_job`
  - `cancel_job`
  - `get_app_info`
  - `next_events` (batched polling for `topic + payload` events)
- Processing is delegated to `crates/backend` (which wraps `crates/job_runner` and `crates/core`).

Notes:
- Frontend UI (React/Vite) is not scaffolded yet.
- Event delivery currently uses batched polling (`next_events`) to keep integration deterministic.
