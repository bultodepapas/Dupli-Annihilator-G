#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use dedupe_backend::{
    AppInfo, BackendService, CancelJobRequest, CancelJobResponse, CheckWordResponse, CommandError,
    EmittedEvent, FrequencyRequest, FrequencyResponse, FuzzyClusterRequest, FuzzyClusterResponse,
    LoadCheckerResponse, NgramRequest, NgramResponse, RuntimeState, SetOpRequest, SetOpResponse,
    StartJobRequest, StartJobResponse,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use std::fs;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenExternalUrlRequest {
    url: String,
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

#[tauri::command]
fn load_wordlist_for_checker(
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<LoadCheckerResponse, CommandError> {
    state.backend.load_wordlist_for_checker(path)
}

#[tauri::command]
fn check_word(
    state: tauri::State<'_, AppState>,
    word: String,
) -> Result<CheckWordResponse, CommandError> {
    state.backend.check_word(word)
}

#[tauri::command]
fn run_frequency_analysis(
    state: tauri::State<'_, AppState>,
    req: FrequencyRequest,
) -> Result<FrequencyResponse, CommandError> {
    state.backend.run_frequency_analysis(req)
}

#[tauri::command]
fn run_set_op(
    state: tauri::State<'_, AppState>,
    req: SetOpRequest,
) -> Result<SetOpResponse, CommandError> {
    state.backend.run_set_op(req)
}

#[tauri::command]
fn run_ngram_extract(
    state: tauri::State<'_, AppState>,
    req: NgramRequest,
) -> Result<NgramResponse, CommandError> {
    state.backend.run_ngram_extract(req)
}

#[tauri::command]
fn run_fuzzy_cluster(
    state: tauri::State<'_, AppState>,
    req: FuzzyClusterRequest,
) -> Result<FuzzyClusterResponse, CommandError> {
    state.backend.run_fuzzy_cluster(req)
}

#[tauri::command]
fn open_external_url(req: OpenExternalUrlRequest) -> Result<(), String> {
    let url = req.url.trim();
    const ALLOWED_PREFIX: &str = "https://github.com/bultodepapas/Dupli-Annihilator-G/releases";

    if !url.starts_with(ALLOWED_PREFIX) {
        return Err("blocked external URL".to_string());
    }

    open_path_with_default_app(url)
}

fn open_path_with_default_app(path: &str) -> Result<(), String> {
    open::that(path).map_err(|e| format!("failed to open '{}': {e}", path))
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
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
            export_summary_json,
            open_external_url,
            load_wordlist_for_checker,
            check_word,
            run_frequency_analysis,
            run_set_op,
            run_ngram_extract,
            run_fuzzy_cluster
        ])
        .run(tauri::generate_context!())
        .expect("failed to run tauri app");
}
