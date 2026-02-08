#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use dedupe_backend::{
    AppInfo, BackendService, CancelJobRequest, CancelJobResponse, CommandError, EmittedEvent,
    StartJobRequest, StartJobResponse,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;

struct AppState {
    backend: BackendService,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NextEventsRequest {
    max_events: usize,
    timeout_ms: u64,
}

#[tauri::command]
fn start_job(
    state: tauri::State<'_, AppState>,
    req: StartJobRequest,
) -> Result<StartJobResponse, CommandError> {
    state.backend.start_job(req)
}

#[tauri::command]
fn cancel_job(state: tauri::State<'_, AppState>, req: CancelJobRequest) -> CancelJobResponse {
    state.backend.cancel_job(req)
}

#[tauri::command]
fn get_app_info(state: tauri::State<'_, AppState>) -> AppInfo {
    state.backend.get_app_info()
}

#[tauri::command]
fn next_events(state: tauri::State<'_, AppState>, req: NextEventsRequest) -> Vec<EmittedEvent> {
    let max_events = req.max_events.clamp(1, 256);
    let timeout_ms = req.timeout_ms.min(5_000);
    state
        .backend
        .next_emitted_events_batch(Duration::from_millis(timeout_ms), max_events)
}

fn main() {
    tauri::Builder::default()
        .manage(AppState {
            backend: BackendService::new(),
        })
        .invoke_handler(tauri::generate_handler![
            start_job,
            cancel_job,
            get_app_info,
            next_events
        ])
        .run(tauri::generate_context!())
        .expect("failed to run tauri app");
}
