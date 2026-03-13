use anyhow::Context;
use dedupe_core::{
    set_op, token_frequency, Config, DiskAlphabeticalMode, Mode, NoCancel, NoProgress,
    OutputOrdering, SetOp, WordChecker,
};
use dedupe_job_runner::{JobError, JobEvent, JobId, JobManager};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

const COMPATIBLE_EXTENSIONS: &[&str] = &["txt", "csv", "tsv", "log", "pdf", "epub"];

/// Typed error for invalid user-supplied input paths.
///
/// Wrapped in `anyhow::Error` by [`expand_path`] so that
/// [`map_anyhow_to_command_error`] can `downcast_ref` without string matching.
#[derive(Debug, thiserror::Error)]
enum InputError {
    #[error("no compatible files found in folder: {0}")]
    NoCompatibleFiles(String),
}

fn collect_compatible_files(root: &std::path::Path, out: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    // Iterative BFS/DFS using an explicit stack — avoids stack overflow on
    // deeply nested directory trees that would blow the call stack recursively.
    let mut dirs = vec![root.to_path_buf()];
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(&dir)
            .with_context(|| format!("cannot read directory: {}", dir.display()))?
        {
            let entry =
                entry.with_context(|| format!("error reading entry in: {}", dir.display()))?;
            let path = entry.path();
            let file_type = entry
                .file_type()
                .with_context(|| format!("cannot stat: {}", path.display()))?;
            if file_type.is_dir() {
                dirs.push(path);
            } else if file_type.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if COMPATIBLE_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()) {
                        out.push(path);
                    }
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
            return Err(InputError::NoCompatibleFiles(path.display().to_string()).into());
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
    #[serde(default)]
    pub drop_length_min: Option<usize>,
    #[serde(default)]
    pub drop_length_max: Option<usize>,
    #[serde(default = "default_disk_buckets")]
    pub disk_buckets: usize,
    #[serde(default = "default_disk_alphabetical_mode")]
    pub disk_alphabetical_mode: ApiDiskAlphabeticalMode,
    #[serde(default = "default_disk_run_bytes")]
    pub disk_run_bytes: usize,
    #[serde(default)]
    pub per_file_stats: bool,
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

/// Request payload for [`BackendService::run_frequency_analysis`].
///
/// Only the filter fields that `token_frequency` uses are exposed here.
/// Mode, ordering, and output path are irrelevant (no file is written).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrequencyRequest {
    /// Input file paths or directory paths (expanded with the same rules as
    /// [`StartJobConfig`]).
    pub inputs: Vec<String>,
    /// Strip leading/trailing whitespace from each token (default: `true`).
    #[serde(default = "default_trim")]
    pub trim: bool,
    /// Drop empty tokens after trimming (default: `true`).
    #[serde(default = "default_drop_empty")]
    pub drop_empty: bool,
    /// Drop tokens whose character length is within `[min, max]` (inclusive).
    #[serde(default)]
    pub drop_length_min: Option<usize>,
    #[serde(default)]
    pub drop_length_max: Option<usize>,
    /// Return only the N most-frequent tokens.  `None` returns all tokens.
    #[serde(default)]
    pub top_n: Option<usize>,
}

/// One entry in the frequency table.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FrequencyEntry {
    pub token: String,
    pub count: u64,
}

/// Response payload for [`BackendService::run_frequency_analysis`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FrequencyResponse {
    /// Frequency table, sorted descending by count, ties broken alphabetically.
    /// Truncated to `top_n` entries when that field was set in the request.
    pub entries: Vec<FrequencyEntry>,
    /// Total tokens observed across all input files (before any filters).
    pub tokens_seen: u64,
    /// Total distinct tokens found (= full table length before `top_n` cap).
    pub unique_tokens: u64,
}

// ── Set-operation types ───────────────────────────────────────────────────────

/// Which relational set operation to perform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiSetOp {
    /// Tokens in the left group not present in the right.
    Diff,
    /// Tokens present in both groups.
    Intersect,
    /// All unique tokens across both groups.
    Union,
}

/// Request payload for [`BackendService::run_set_op`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetOpRequest {
    /// Left-side input file or directory paths.
    pub left: Vec<String>,
    /// Right-side input file or directory paths.
    pub right: Vec<String>,
    /// Which operation to compute.
    pub op: ApiSetOp,
    /// Output file path (required — the result is written to disk).
    pub output: String,
    /// Allow overwriting an existing output file without error.
    #[serde(default)]
    pub allow_overwrite: bool,
    /// Output token ordering.
    #[serde(default = "default_ordering")]
    pub ordering: ApiOrdering,
    /// Strip leading/trailing whitespace from each token (default: `true`).
    #[serde(default = "default_trim")]
    pub trim: bool,
    /// Drop empty tokens after trimming (default: `true`).
    #[serde(default = "default_drop_empty")]
    pub drop_empty: bool,
    /// Drop tokens whose character length is within `[min, max]` (inclusive).
    #[serde(default)]
    pub drop_length_min: Option<usize>,
    #[serde(default)]
    pub drop_length_max: Option<usize>,
    /// Output separator string (default: `"\n"`).
    #[serde(default = "default_separator")]
    pub output_separator: String,
    /// When `true`, interpret `\n`, `\t`, etc. in `output_separator`.
    #[serde(default)]
    pub interpret_separator_escapes: bool,
}

/// Response payload for [`BackendService::run_set_op`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetOpResponse {
    /// Number of unique tokens written to the output file.
    pub unique_tokens: u64,
    /// Total tokens read from both left and right inputs (before filters).
    pub tokens_seen: u64,
    /// Wall-clock milliseconds spent on the operation.
    pub elapsed_ms: u64,
    /// The resolved output path (echoed back for convenience).
    pub output_path: String,
}

pub struct BackendService {
    manager: JobManager,
    checker: Mutex<Option<WordChecker>>,
}

impl Default for BackendService {
    fn default() -> Self {
        Self::new()
    }
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
        cfg.validate().map_err(|e| CommandError {
            category: "invalid_config".to_string(),
            message: e.to_string(),
            detail: None,
        })?;
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
        *self.checker.lock().map_err(|_| CommandError {
            category: "internal_error".to_string(),
            message: "wordlist checker lock poisoned".to_string(),
            detail: None,
        })? = Some(wc);
        Ok(LoadCheckerResponse { word_count })
    }

    pub fn check_word(&self, word: String) -> Result<CheckWordResponse, CommandError> {
        let guard = self.checker.lock().map_err(|_| CommandError {
            category: "internal_error".to_string(),
            message: "wordlist checker lock poisoned".to_string(),
            detail: None,
        })?;
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

    /// Run a synchronous frequency analysis over the given inputs and return
    /// a ranked token → count table.
    ///
    /// This call **blocks** the calling thread until all inputs are read —
    /// the same behaviour as [`Self::load_wordlist_for_checker`].  For very
    /// large corpora consider keeping the input list small or using a
    /// background job via [`Self::start_job`] with a future async variant.
    pub fn run_frequency_analysis(
        &self,
        req: FrequencyRequest,
    ) -> Result<FrequencyResponse, CommandError> {
        // Expand and validate inputs (identical path to into_core_config).
        let mut inputs: Vec<std::path::PathBuf> = Vec::new();
        for raw in req.inputs {
            let files =
                expand_path(std::path::PathBuf::from(raw)).map_err(map_anyhow_to_command_error)?;
            inputs.extend(files);
        }
        if inputs.is_empty() {
            return Err(CommandError {
                category: "invalid_config".to_string(),
                message: "no input files provided".to_string(),
                detail: None,
            });
        }

        // Build a minimal Config — output path intentionally left empty because
        // token_frequency never writes a file and does not call Config::validate().
        let config = Config {
            inputs,
            trim: req.trim,
            drop_empty: req.drop_empty,
            drop_length_min: req.drop_length_min,
            drop_length_max: req.drop_length_max,
            ..Config::default()
        };

        let raw =
            token_frequency(&config, &NoProgress, &NoCancel).map_err(map_anyhow_to_command_error)?;

        // Derive aggregate stats before consuming the vec.
        let unique_tokens = raw.len() as u64;
        let tokens_seen: u64 = raw.iter().map(|(_, c)| c).sum();

        let take = req.top_n.unwrap_or(usize::MAX);
        let entries: Vec<FrequencyEntry> = raw
            .into_iter()
            .take(take)
            .map(|(token, count)| FrequencyEntry {
                // Box<str> → String: From<Box<str>> for String is in std.
                token: String::from(token),
                count,
            })
            .collect();

        Ok(FrequencyResponse {
            entries,
            tokens_seen,
            unique_tokens,
        })
    }

    pub fn run_set_op(&self, req: SetOpRequest) -> Result<SetOpResponse, CommandError> {
        // Expand left inputs.
        let mut left: Vec<PathBuf> = Vec::new();
        for raw in req.left {
            let files = expand_path(PathBuf::from(raw)).map_err(map_anyhow_to_command_error)?;
            left.extend(files);
        }
        if left.is_empty() {
            return Err(CommandError {
                category: "invalid_config".to_string(),
                message: "left: no input files provided".to_string(),
                detail: None,
            });
        }

        // Expand right inputs (allowed to be empty — Diff with empty right = identity).
        let mut right: Vec<PathBuf> = Vec::new();
        for raw in req.right {
            let files = expand_path(PathBuf::from(raw)).map_err(map_anyhow_to_command_error)?;
            right.extend(files);
        }

        let output_path = PathBuf::from(&req.output);
        if !req.allow_overwrite && output_path.exists() {
            return Err(CommandError {
                category: "output_exists".to_string(),
                message: format!("output file already exists: {}", output_path.display()),
                detail: None,
            });
        }

        let output_separator = if req.interpret_separator_escapes {
            parse_escaped_separator(&req.output_separator)
        } else {
            req.output_separator
        };

        let config = Config {
            inputs: left,
            output: output_path.clone(),
            output_separator,
            ordering: map_ordering(req.ordering),
            trim: req.trim,
            drop_empty: req.drop_empty,
            drop_length_min: req.drop_length_min,
            drop_length_max: req.drop_length_max,
            ..Config::default()
        };

        let op = match req.op {
            ApiSetOp::Diff => SetOp::Diff,
            ApiSetOp::Intersect => SetOp::Intersect,
            ApiSetOp::Union => SetOp::Union,
        };

        let stats =
            set_op(&right, op, &config, &NoProgress, &NoCancel)
                .map_err(map_anyhow_to_command_error)?;

        Ok(SetOpResponse {
            unique_tokens: stats.unique_tokens,
            tokens_seen: stats.tokens_seen,
            elapsed_ms: stats.elapsed.as_millis() as u64,
            output_path: output_path.to_string_lossy().into_owned(),
        })
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
            drop_length_min: self.drop_length_min,
            drop_length_max: self.drop_length_max,
            disk_buckets: self.disk_buckets,
            disk_alphabetical_mode: map_disk_mode(self.disk_alphabetical_mode),
            disk_run_bytes: self.disk_run_bytes,
            per_file_stats: self.per_file_stats,
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
    let category = if err.downcast_ref::<JobError>().is_some() {
        "job_busy"
    } else if err.downcast_ref::<InputError>().is_some() {
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
