use dedupe_core::{Config, DiskAlphabeticalMode, Mode, OutputOrdering};
use dedupe_job_runner::{JobEvent, JobId, JobManager};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

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

#[derive(Debug, Default)]
pub struct BackendService {
    manager: JobManager,
}

impl BackendService {
    pub fn new() -> Self {
        Self {
            manager: JobManager::new(),
        }
    }

    pub fn start_job(&self, req: StartJobRequest) -> Result<StartJobResponse, CommandError> {
        let cfg = req
            .config
            .into_core_config()
            .map_err(map_anyhow_to_command_error)?;
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
        }
    }

    pub fn try_next_emitted_event(&self) -> Option<EmittedEvent> {
        self.manager.try_next_event().map(EmittedEvent::from)
    }

    pub fn next_emitted_event_timeout(&self, timeout: Duration) -> Option<EmittedEvent> {
        self.manager
            .next_event_timeout(timeout)
            .map(EmittedEvent::from)
    }

    pub fn drain_emitted_events(&self) -> Vec<EmittedEvent> {
        self.manager
            .drain_events()
            .into_iter()
            .map(EmittedEvent::from)
            .collect()
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
            inputs: self.inputs.into_iter().map(PathBuf::from).collect(),
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
