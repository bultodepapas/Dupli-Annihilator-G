use anyhow::Context;
use dedupe_core::{Config, DiskAlphabeticalMode, Mode, OutputOrdering, WordChecker};
use dedupe_job_runner::{JobEvent, JobId, JobManager};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

const COMPATIBLE_EXTENSIONS: &[&str] = &["txt", "csv", "tsv", "log", "pdf"];

fn collect_compatible_files(dir: &std::path::Path, out: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(dir)
        .with_context(|| format!("cannot read directory: {}", dir.display()))?
    {
        let entry =
            entry.with_context(|| format!("error reading entry in: {}", dir.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("cannot stat: {}", path.display()))?;
        if file_type.is_dir() {
            collect_compatible_files(&path, out)?;
        } else if file_type.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if COMPATIBLE_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()) {
                    out.push(path);
                }
            }
        }
    }
    Ok(())
}

fn expand_path(path: PathBuf) -> anyhow::Result<Vec<PathBuf>> {
    let meta = std::fs::metadata(&path)
        .with_context(|| format!("cannot access path: {}", path.display()))?;
    if meta.is_file() {
        return Ok(vec![path]);
    }
    if meta.is_dir() {
        let mut found: Vec<PathBuf> = Vec::new();
        collect_compatible_files(&path, &mut found)?;
        found.sort();
        if found.is_empty() {
            anyhow::bail!("no compatible files found in folder: {}", path.display());
        }
        return Ok(found);
    }
    anyhow::bail!("path is not a file or directory: {}", path.display());
}

pub use dedupe_job_runner::JobEvent as BackendJobEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiMode {
    Auto,
    Ram,
    Disk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiOrdering {
    PreserveFirstSeen,
    Alphabetical,
    UnorderedFast,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiDiskAlphabeticalMode {
    FastBucketLocal,
    GlobalPerfect,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartJobConfig {
    pub inputs: Vec<String>,
    pub output: String,
    #[serde(default)]
    pub allow_overwrite: bool,
    #[serde(default = "default_separator")]
    pub output_separator: String,
    #[serde(default)]
    pub interpret_separator_escapes: bool,
    #[serde(default = "default_mode")]
    pub mode: ApiMode,
    #[serde(default = "default_ordering")]
    pub ordering: ApiOrdering,
    #[serde(default = "default_trim")]
    pub trim: bool,
    #[serde(default = "default_drop_empty")]
    pub drop_empty: bool,
    #[serde(default = "default_disk_buckets")]
    pub disk_buckets: usize,
    #[serde(default = "default_disk_alphabetical_mode")]
    pub disk_alphabetical_mode: ApiDiskAlphabeticalMode,
    #[serde(default = "default_disk_run_bytes")]
    pub disk_run_bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartJobRequest {
    pub config: StartJobConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StartJobResponse {
    pub job_id: JobId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelJobRequest {
    pub job_id: JobId,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CancelJobResponse {
    pub acknowledged: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub app_name: String,
    pub app_version: String,
    pub backend_version: String,
    pub update_channel: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeState {
    pub is_running: bool,
    pub active_job_id: Option<JobId>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub category: String,
    pub message: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EmittedEvent {
    pub topic: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LoadCheckerResponse {
    pub word_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CheckWordResponse {
    pub found: bool,
    pub word_count: usize,
}

pub struct BackendService {
    manager: JobManager,
    checker: Mutex<Option<WordChecker>>,
}

impl BackendService {
    pub fn new() -> Self {
        Self {
            manager: JobManager::new(),
            checker: Mutex::new(None),
        }
    }

    pub fn start_job(&self, req: StartJobRequest) -> Result<StartJobResponse, CommandError> {
        let allow_overwrite = req.config.allow_overwrite;
        let cfg = req
            .config
            .into_core_config()
            .map_err(map_anyhow_to_command_error)?;
        cfg.validate().map_err(map_anyhow_to_command_error)?;
        if !allow_overwrite && cfg.output.exists() {
            return Err(CommandError {
                category: "output_exists".to_string(),
                message: format!("output file already exists: {}", cfg.output.display()),
                detail: None,
            });
        }
        let job_id = self
            .manager
            .start_job(cfg)
            .map_err(map_anyhow_to_command_error)?;
        Ok(StartJobResponse { job_id })
    }

    pub fn cancel_job(&self, req: CancelJobRequest) -> CancelJobResponse {
        let acknowledged = self.manager.cancel_job(req.job_id);
        CancelJobResponse { acknowledged }
    }

    pub fn get_app_info(&self) -> AppInfo {
        AppInfo {
            app_name: "Dupli-Annihilator-G".to_string(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            backend_version: env!("CARGO_PKG_VERSION").to_string(),
            update_channel: resolve_update_channel(),
        }
    }

    pub fn get_runtime_state(&self) -> RuntimeState {
        RuntimeState {
            is_running: self.manager.is_running(),
            active_job_id: self.manager.active_job_id(),
        }
    }

    pub fn load_wordlist_for_checker(
        &self,
        path: String,
    ) -> Result<LoadCheckerResponse, CommandError> {
        let wc = WordChecker::load(std::path::Path::new(&path))
            .map_err(map_anyhow_to_command_error)?;
        let word_count = wc.len();
        *self.checker.lock().unwrap() = Some(wc);
        Ok(LoadCheckerResponse { word_count })
    }

    pub fn check_word(&self, word: String) -> Result<CheckWordResponse, CommandError> {
        let guard = self.checker.lock().unwrap();
        match guard.as_ref() {
            None => Err(CommandError {
                category: "checker_not_loaded".to_string(),
                message: "No wordlist loaded. Call load_wordlist_for_checker first.".to_string(),
                detail: None,
            }),
            Some(wc) => Ok(CheckWordResponse {
                found: wc.contains(&word),
                word_count: wc.len(),
            }),
        }
    }

    pub fn try_next_emitted_event(&self) -> Option<EmittedEvent> {
        self.try_next_event().map(EmittedEvent::from)
    }

    pub fn next_emitted_event_timeout(&self, timeout: Duration) -> Option<EmittedEvent> {
        self.next_event_timeout(timeout).map(EmittedEvent::from)
    }

    pub fn drain_emitted_events(&self) -> Vec<EmittedEvent> {
        self.drain_events()
            .into_iter()
            .map(EmittedEvent::from)
            .collect()
    }

    pub fn next_emitted_events_batch(
        &self,
        timeout: Duration,
        max_events: usize,
    ) -> Vec<EmittedEvent> {
        if max_events == 0 {
            return Vec::new();
        }

        let mut out = Vec::with_capacity(max_events);
        let Some(first) = self.next_event_timeout(timeout) else {
            return out;
        };
        out.push(EmittedEvent::from(first));

        while out.len() < max_events {
            let Some(next) = self.try_next_event() else {
                break;
            };
            out.push(EmittedEvent::from(next));
        }

        out
    }

    pub fn try_next_event(&self) -> Option<JobEvent> {
        self.manager.try_next_event()
    }

    pub fn next_event_timeout(&self, timeout: Duration) -> Option<JobEvent> {
        self.manager.next_event_timeout(timeout)
    }

    pub fn drain_events(&self) -> Vec<JobEvent> {
        self.manager.drain_events()
    }
}

fn resolve_update_channel() -> String {
    let from_env = std::env::var("DUPLI_UPDATE_CHANNEL").unwrap_or_default();
    let normalized = from_env.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        "stable".to_string()
    } else {
        normalized
    }
}

impl StartJobConfig {
    fn into_core_config(self) -> anyhow::Result<Config> {
        let output_separator = if self.interpret_separator_escapes {
            parse_escaped_separator(&self.output_separator)
        } else {
            self.output_separator
        };

        Ok(Config {
            inputs: {
                let mut expanded: Vec<PathBuf> = Vec::new();
                for raw in self.inputs {
                    let path = PathBuf::from(raw);
                    let files = expand_path(path)?;
                    expanded.extend(files);
                }
                expanded
            },
            output: PathBuf::from(self.output),
            output_separator,
            mode: map_mode(self.mode),
            ordering: map_ordering(self.ordering),
            trim: self.trim,
            drop_empty: self.drop_empty,
            disk_buckets: self.disk_buckets,
            disk_alphabetical_mode: map_disk_mode(self.disk_alphabetical_mode),
            disk_run_bytes: self.disk_run_bytes,
        })
    }
}

impl From<JobEvent> for EmittedEvent {
    fn from(value: JobEvent) -> Self {
        let topic = value.topic().to_string();
        let payload = value.to_json_value();
        Self { topic, payload }
    }
}

fn map_mode(mode: ApiMode) -> Mode {
    match mode {
        ApiMode::Auto => Mode::Auto,
        ApiMode::Ram => Mode::Ram,
        ApiMode::Disk => Mode::Disk,
    }
}

fn map_ordering(ordering: ApiOrdering) -> OutputOrdering {
    match ordering {
        ApiOrdering::PreserveFirstSeen => OutputOrdering::PreserveFirstSeen,
        ApiOrdering::Alphabetical => OutputOrdering::Alphabetical,
        ApiOrdering::UnorderedFast => OutputOrdering::UnorderedFast,
    }
}

fn map_disk_mode(mode: ApiDiskAlphabeticalMode) -> DiskAlphabeticalMode {
    match mode {
        ApiDiskAlphabeticalMode::FastBucketLocal => DiskAlphabeticalMode::FastBucketLocal,
        ApiDiskAlphabeticalMode::GlobalPerfect => DiskAlphabeticalMode::GlobalPerfect,
    }
}

fn map_anyhow_to_command_error(err: anyhow::Error) -> CommandError {
    let message = err.to_string();
    let lower = message.to_ascii_lowercase();
    let category = if lower.contains("already running") {
        "job_busy"
    } else if lower.contains("no input files")
        || lower.contains("no compatible files")
        || lower.contains("output path is required")
        || lower.contains("separator")
        || lower.contains("disk_buckets")
        || lower.contains("disk_run_bytes")
    {
        "invalid_config"
    } else {
        "runtime_error"
    };

    CommandError {
        category: category.to_string(),
        message,
        detail: Some(format!("{err:#}")),
    }
}

fn parse_escaped_separator(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }

        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => {
                if matches!(chars.peek(), Some('n')) {
                    chars.next();
                    out.push('\r');
                    out.push('\n');
                } else {
                    out.push('\r');
                }
            }
            Some('f') => out.push('\u{000C}'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }

    out
}

fn default_mode() -> ApiMode {
    ApiMode::Ram
}

fn default_ordering() -> ApiOrdering {
    ApiOrdering::PreserveFirstSeen
}

fn default_trim() -> bool {
    true
}

fn default_drop_empty() -> bool {
    true
}

fn default_disk_buckets() -> usize {
    256
}

fn default_disk_alphabetical_mode() -> ApiDiskAlphabeticalMode {
    ApiDiskAlphabeticalMode::FastBucketLocal
}

fn default_disk_run_bytes() -> usize {
    256 * 1024 * 1024
}

fn default_separator() -> String {
    "\n".to_string()
}
