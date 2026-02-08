#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use dedupe_backend::{
    AppInfo, BackendService, CancelJobRequest, CancelJobResponse, CommandError, EmittedEvent,
    RuntimeState, StartJobRequest, StartJobResponse,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use std::{fs, process::Command};

struct AppState {
    backend: BackendService,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NextEventsRequest {
    max_events: usize,
    timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenPathRequest {
    path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportSummaryJsonRequest {
    path: String,
    content: String,
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
fn get_runtime_state(state: tauri::State<'_, AppState>) -> RuntimeState {
    state.backend.get_runtime_state()
}

#[tauri::command]
fn path_exists(path: String) -> bool {
    std::path::Path::new(&path).exists()
}

#[tauri::command]
fn default_output_path() -> String {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"));
    let home = home.map(PathBuf::from);
    let desktop = home
        .as_ref()
        .map(|p| p.join("Desktop"))
        .filter(|p| p.is_dir());

    let base = if cfg!(target_os = "windows") || cfg!(target_os = "macos") {
        desktop.or(home)
    } else {
        home
    }
    .or_else(|| std::env::current_dir().ok())
    .unwrap_or_else(|| PathBuf::from("."));

    base.join("ready.csv").to_string_lossy().into_owned()
}

#[tauri::command]
fn next_events(state: tauri::State<'_, AppState>, req: NextEventsRequest) -> Vec<EmittedEvent> {
    let max_events = req.max_events.clamp(1, 256);
    let timeout_ms = req.timeout_ms.min(5_000);
    state
        .backend
        .next_emitted_events_batch(Duration::from_millis(timeout_ms), max_events)
}

#[tauri::command]
fn open_output(req: OpenPathRequest) -> Result<(), String> {
    open_path_with_default_app(&req.path)
}

#[tauri::command]
fn open_output_folder(req: OpenPathRequest) -> Result<(), String> {
    let path = PathBuf::from(req.path);
    let Some(parent) = path.parent() else {
        return Err("output path has no parent directory".to_string());
    };
    open_path_with_default_app(&parent.to_string_lossy())
}

#[tauri::command]
fn export_summary_json(req: ExportSummaryJsonRequest) -> Result<(), String> {
    fs::write(&req.path, req.content)
        .map_err(|e| format!("failed to write summary JSON '{}': {e}", req.path))
}

fn open_path_with_default_app(path: &str) -> Result<(), String> {
    let status = if cfg!(target_os = "windows") {
        Command::new("cmd").args(["/C", "start", "", path]).status()
    } else if cfg!(target_os = "macos") {
        Command::new("open").arg(path).status()
    } else {
        Command::new("xdg-open").arg(path).status()
    }
    .map_err(|e| format!("failed to open path '{}': {e}", path))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("open command failed for '{}': exit={status}", path))
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            backend: BackendService::new(),
        })
        .invoke_handler(tauri::generate_handler![
            start_job,
            cancel_job,
            get_app_info,
            get_runtime_state,
            path_exists,
            default_output_path,
            next_events,
            open_output,
            open_output_folder,
            export_summary_json
        ])
        .run(tauri::generate_context!())
        .expect("failed to run tauri app");
}
