use anyhow::anyhow;
use dedupe_core::{
    is_canceled_error, run_with_control, CancellationToken, Config, DiskAlphabeticalMode, Mode,
    OutputOrdering, ProgressEvent, ProgressSink, Stats,
};
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc::{self, Receiver, Sender},
    Arc, Mutex,
};
use std::time::{Duration, Instant};

pub type JobId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobState {
    Idle,
    Running,
    Finalizing,
    Done,
    Error,
    Canceled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatsSnapshot {
    pub files: usize,
    pub tokens_seen: u64,
    pub unique_tokens: u64,
    pub duplicates: u64,
    pub elapsed_ms: u128,
}

impl StatsSnapshot {
    fn from_stats(stats: Stats) -> Self {
        Self {
            files: stats.files,
            tokens_seen: stats.tokens_seen,
            unique_tokens: stats.unique_tokens,
            duplicates: stats.duplicates,
            elapsed_ms: stats.elapsed.as_millis(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunTerminalStatus {
    Success,
    Error,
    Canceled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunSummary {
    pub job_id: JobId,
    pub status: RunTerminalStatus,
    pub started_at: String,
    pub finished_at: String,
    pub files_total: usize,
    pub files_done: usize,
    pub tokens_seen: u64,
    pub unique_tokens: u64,
    pub duplicates: u64,
    pub reduction_pct: f64,
    pub uniq_pct: f64,
    pub input_bytes_total: u64,
    pub output_path: String,
    pub output_bytes: u64,
    pub mode: String,
    pub mode_effective: String,
    pub ordering: String,
    pub disk_alphabetical_mode: Option<String>,
    pub disk_buckets: Option<usize>,
    pub disk_run_bytes: Option<usize>,
    pub trim: bool,
    pub drop_empty: bool,
    pub output_separator_raw: String,
    pub output_separator_preview: String,
    pub elapsed_ms: u128,
    pub avg_throughput_tps: u64,
    pub peak_throughput_tps: Option<u64>,
    pub stage_durations_ms: Option<BTreeMap<String, u128>>,
    pub temp_bytes_total: Option<u64>,
    pub warnings: Vec<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone)]
struct BridgeReport {
    snapshot: ProgressSnapshot,
    stage_durations_ms: BTreeMap<String, u128>,
    peak_throughput_tps: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum JobEvent {
    Started {
        job_id: JobId,
    },
    Stage {
        job_id: JobId,
        stage: String,
    },
    Progress {
        job_id: JobId,
        stage: Option<String>,
        files_done: usize,
        files_total: usize,
        progress_ppm: Option<u32>,
        tokens_seen: u64,
        unique_tokens: u64,
        duplicates: u64,
        throughput_tps: u64,
        elapsed_ms: u128,
        eta_ms: Option<u128>,
    },
    Done {
        job_id: JobId,
        stats: StatsSnapshot,
    },
    Error {
        job_id: JobId,
        message: String,
    },
    Canceled {
        job_id: JobId,
    },
    Summary {
        job_id: JobId,
        summary: RunSummary,
    },
}

impl JobEvent {
    pub fn topic(&self) -> &'static str {
        match self {
            JobEvent::Started { .. } => "job://started",
            JobEvent::Stage { .. } => "job://stage",
            JobEvent::Progress { .. } => "job://progress",
            JobEvent::Done { .. } => "job://done",
            JobEvent::Error { .. } => "job://error",
            JobEvent::Canceled { .. } => "job://canceled",
            JobEvent::Summary { .. } => "job://summary",
        }
    }

    pub fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_else(|_| serde_json::json!({}))
    }
}

#[derive(Debug)]
struct ActiveJob {
    id: JobId,
    cancel: CancellationToken,
    done: Arc<AtomicBool>,
}

#[derive(Debug)]
struct Inner {
    next_id: AtomicU64,
    active: Mutex<Option<ActiveJob>>,
    tx: Sender<JobEvent>,
    rx: Mutex<Receiver<JobEvent>>,
}

#[derive(Debug, Clone)]
pub struct JobManager {
    inner: Arc<Inner>,
}

impl Default for JobManager {
    fn default() -> Self {
        Self::new()
    }
}

impl JobManager {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            inner: Arc::new(Inner {
                next_id: AtomicU64::new(0),
                active: Mutex::new(None),
                tx,
                rx: Mutex::new(rx),
            }),
        }
    }

    pub fn start_job(&self, config: Config) -> anyhow::Result<JobId> {
        let mut active = self
            .inner
            .active
            .lock()
            .map_err(|_| anyhow!("active job lock poisoned"))?;
        Self::prune_finished_locked(&mut active);
        if active.is_some() {
            return Err(anyhow!("another job is already running"));
        }

        let job_id = self.inner.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let cancel = CancellationToken::new();
        let done = Arc::new(AtomicBool::new(false));
        *active = Some(ActiveJob {
            id: job_id,
            cancel: cancel.clone(),
            done: Arc::clone(&done),
        });
        drop(active);

        let tx = self.inner.tx.clone();
        let inner = Arc::clone(&self.inner);
        std::thread::spawn(move || {
            let _ = tx.send(JobEvent::Started { job_id });

            let started_at = Utc::now();
            let input_bytes_total = sum_existing_input_bytes(&config);
            let sink = Arc::new(BridgeSink::new(job_id, tx.clone()));
            let result = run_with_control(&config, Arc::clone(&sink), cancel.clone());
            let finished_at = Utc::now();
            let bridge = sink.finalize_report();
            let output_bytes = file_len_or_zero(&config.output);

            let (status, stats_for_summary, error_message) = match result {
                Ok(stats) => {
                    let snap = StatsSnapshot::from_stats(stats);
                    let _ = tx.send(JobEvent::Done {
                        job_id,
                        stats: snap.clone(),
                    });
                    (RunTerminalStatus::Success, snap, None)
                }
                Err(err) => {
                    if is_canceled_error(&err) || cancel.is_canceled() {
                        let _ = tx.send(JobEvent::Canceled { job_id });
                        (RunTerminalStatus::Canceled, bridge.snapshot.to_stats_snapshot(), None)
                    } else {
                        let message = format!("{err:#}");
                        let _ = tx.send(JobEvent::Error {
                            job_id,
                            message: message.clone(),
                        });
                        (
                            RunTerminalStatus::Error,
                            bridge.snapshot.to_stats_snapshot(),
                            Some(message),
                        )
                    }
                }
            };

            let summary = build_run_summary(
                job_id,
                status,
                started_at,
                finished_at,
                &config,
                &bridge,
                &stats_for_summary,
                input_bytes_total,
                output_bytes,
                error_message,
            );
            let _ = tx.send(JobEvent::Summary { job_id, summary });

            done.store(true, Ordering::Release);
            if let Ok(mut current) = inner.active.lock() {
                if current.as_ref().map(|job| job.id) == Some(job_id) {
                    *current = None;
                }
            }
        });

        Ok(job_id)
    }

    pub fn cancel_job(&self, job_id: JobId) -> bool {
        let mut active = match self.inner.active.lock() {
            Ok(lock) => lock,
            Err(_) => return false,
        };
        Self::prune_finished_locked(&mut active);

        if let Some(job) = active.as_ref() {
            if job.id == job_id {
                job.cancel.cancel();
                return true;
            }
        }
        false
    }

    pub fn active_job_id(&self) -> Option<JobId> {
        let mut active = self.inner.active.lock().ok()?;
        Self::prune_finished_locked(&mut active);
        active.as_ref().map(|job| job.id)
    }

    pub fn is_running(&self) -> bool {
        self.active_job_id().is_some()
    }

    pub fn try_next_event(&self) -> Option<JobEvent> {
        let rx = self.inner.rx.lock().ok()?;
        rx.try_recv().ok()
    }

    pub fn next_event_timeout(&self, timeout: Duration) -> Option<JobEvent> {
        let rx = self.inner.rx.lock().ok()?;
        rx.recv_timeout(timeout).ok()
    }

    pub fn drain_events(&self) -> Vec<JobEvent> {
        let mut out = Vec::new();
        while let Some(event) = self.try_next_event() {
            out.push(event);
        }
        out
    }

    fn prune_finished_locked(active: &mut Option<ActiveJob>) {
        if active
            .as_ref()
            .map(|job| job.done.load(Ordering::Acquire))
            .unwrap_or(false)
        {
            *active = None;
        }
    }
}

#[derive(Debug, Default, Clone)]
struct ProgressSnapshot {
    stage: Option<String>,
    files_done: usize,
    files_total: usize,
    progress_ppm: Option<u32>,
    tokens_seen: u64,
    unique_tokens: u64,
    duplicates: u64,
    throughput_tps: u64,
    elapsed_ms: u128,
    eta_ms: Option<u128>,
}

impl ProgressSnapshot {
    fn to_stats_snapshot(&self) -> StatsSnapshot {
        StatsSnapshot {
            files: self.files_total,
            tokens_seen: self.tokens_seen,
            unique_tokens: self.unique_tokens,
            duplicates: self.duplicates,
            elapsed_ms: self.elapsed_ms,
        }
    }
}

#[derive(Debug)]
struct BridgeState {
    snapshot: ProgressSnapshot,
    started_at: Instant,
    last_emit_at: Instant,
    last_tp_sample_at: Instant,
    last_tp_tokens: u64,
    ewma_tps: f64,
    peak_tps: u64,
    current_stage: Option<String>,
    current_stage_started_at: Option<Instant>,
    stage_durations_ms: BTreeMap<String, u128>,
}

#[derive(Debug)]
struct BridgeSink {
    job_id: JobId,
    tx: Sender<JobEvent>,
    state: Mutex<BridgeState>,
}

impl BridgeSink {
    const MIN_PROGRESS_INTERVAL: Duration = Duration::from_millis(125);
    const EWMA_ALPHA: f64 = 0.25;

    fn new(job_id: JobId, tx: Sender<JobEvent>) -> Self {
        let now = Instant::now();
        Self {
            job_id,
            tx,
            state: Mutex::new(BridgeState {
                snapshot: ProgressSnapshot::default(),
                started_at: now,
                last_emit_at: now,
                last_tp_sample_at: now,
                last_tp_tokens: 0,
                ewma_tps: 0.0,
                peak_tps: 0,
                current_stage: None,
                current_stage_started_at: None,
                stage_durations_ms: BTreeMap::new(),
            }),
        }
    }

    fn close_current_stage(state: &mut BridgeState, now: Instant) {
        let Some(stage) = state.current_stage.take() else {
            return;
        };
        let Some(started) = state.current_stage_started_at.take() else {
            return;
        };
        let elapsed = now.duration_since(started).as_millis();
        let entry = state.stage_durations_ms.entry(stage).or_insert(0);
        *entry += elapsed;
    }

    fn finalize_report(&self) -> BridgeReport {
        let mut state = match self.state.lock() {
            Ok(lock) => lock,
            Err(_) => {
                return BridgeReport {
                    snapshot: ProgressSnapshot::default(),
                    stage_durations_ms: BTreeMap::new(),
                    peak_throughput_tps: 0,
                };
            }
        };

        let now = Instant::now();
        Self::close_current_stage(&mut state, now);
        Self::update_derived_metrics(&mut state, now);

        BridgeReport {
            snapshot: state.snapshot.clone(),
            stage_durations_ms: state.stage_durations_ms.clone(),
            peak_throughput_tps: state.peak_tps,
        }
    }

    fn emit_progress(&self, snapshot: &ProgressSnapshot) {
        let _ = self.tx.send(JobEvent::Progress {
            job_id: self.job_id,
            stage: snapshot.stage.clone(),
            files_done: snapshot.files_done,
            files_total: snapshot.files_total,
            progress_ppm: snapshot.progress_ppm,
            tokens_seen: snapshot.tokens_seen,
            unique_tokens: snapshot.unique_tokens,
            duplicates: snapshot.duplicates,
            throughput_tps: snapshot.throughput_tps,
            elapsed_ms: snapshot.elapsed_ms,
            eta_ms: snapshot.eta_ms,
        });
    }

    fn update_derived_metrics(state: &mut BridgeState, now: Instant) {
        let elapsed = now.duration_since(state.started_at);
        let elapsed_ms = elapsed.as_millis();
        state.snapshot.elapsed_ms = elapsed_ms;

        let since_sample = now.duration_since(state.last_tp_sample_at);
        if since_sample >= Duration::from_millis(200)
            && state.snapshot.tokens_seen >= state.last_tp_tokens
        {
            let delta_tokens = state.snapshot.tokens_seen - state.last_tp_tokens;
            let sec = since_sample.as_secs_f64();
            if sec > 0.0 {
                let inst_tps = delta_tokens as f64 / sec;
                state.ewma_tps = if state.ewma_tps > 0.0 {
                    (Self::EWMA_ALPHA * inst_tps) + ((1.0 - Self::EWMA_ALPHA) * state.ewma_tps)
                } else {
                    inst_tps
                };
            }
            state.last_tp_tokens = state.snapshot.tokens_seen;
            state.last_tp_sample_at = now;
        }
        state.snapshot.throughput_tps = state.ewma_tps.max(0.0).round() as u64;
        if state.snapshot.throughput_tps > state.peak_tps {
            state.peak_tps = state.snapshot.throughput_tps;
        }

        state.snapshot.progress_ppm =
            Self::estimate_progress_ppm(state.snapshot.files_done, state.snapshot.files_total);

        state.snapshot.eta_ms = if state.snapshot.files_total > 0
            && state.snapshot.files_done > 0
            && state.snapshot.files_done < state.snapshot.files_total
            && elapsed_ms >= 1_000
        {
            let files_per_ms = state.snapshot.files_done as f64 / elapsed_ms as f64;
            if files_per_ms > 0.0 {
                let remaining = (state.snapshot.files_total - state.snapshot.files_done) as f64;
                Some((remaining / files_per_ms).round() as u128)
            } else {
                None
            }
        } else {
            None
        };
    }

    fn estimate_progress_ppm(files_done: usize, files_total: usize) -> Option<u32> {
        if files_total == 0 {
            return None;
        }
        let ppm = ((files_done as u128) * 1_000_000u128) / (files_total as u128);
        Some(ppm.min(1_000_000u128) as u32)
    }
}

impl ProgressSink for BridgeSink {
    fn on_event(&self, event: ProgressEvent) {
        let mut state = match self.state.lock() {
            Ok(lock) => lock,
            Err(_) => return,
        };
        let now = Instant::now();
        let mut force_emit = false;

        match event {
            ProgressEvent::Stage(stage) => {
                Self::close_current_stage(&mut state, now);
                state.current_stage = Some(stage.to_string());
                state.current_stage_started_at = Some(now);
                state.snapshot.stage = Some(stage.to_string());
                let _ = self.tx.send(JobEvent::Stage {
                    job_id: self.job_id,
                    stage: stage.to_string(),
                });
                force_emit = true;
            }
            ProgressEvent::FileStarted { index: _, total } => {
                state.snapshot.files_total = total;
            }
            ProgressEvent::FileFinished { index, total } => {
                state.snapshot.files_done = index;
                state.snapshot.files_total = total;
                force_emit = true;
            }
            ProgressEvent::TokensSeen(v) => {
                state.snapshot.tokens_seen = v;
            }
            ProgressEvent::UniqueTokens(v) => {
                state.snapshot.unique_tokens = v;
            }
            ProgressEvent::Duplicates(v) => {
                state.snapshot.duplicates = v;
            }
        }

        let elapsed_since_emit = now.duration_since(state.last_emit_at);
        if force_emit || elapsed_since_emit >= Self::MIN_PROGRESS_INTERVAL {
            Self::update_derived_metrics(&mut state, now);
            let snapshot_copy = state.snapshot.clone();
            state.last_emit_at = now;
            drop(state);
            self.emit_progress(&snapshot_copy);
        }
    }
}

impl ProgressSink for Arc<BridgeSink> {
    fn on_event(&self, event: ProgressEvent) {
        ProgressSink::on_event(self.as_ref(), event);
    }
}

fn build_run_summary(
    job_id: JobId,
    status: RunTerminalStatus,
    started_at: chrono::DateTime<Utc>,
    finished_at: chrono::DateTime<Utc>,
    config: &Config,
    bridge: &BridgeReport,
    stats: &StatsSnapshot,
    input_bytes_total: u64,
    output_bytes: u64,
    error_message: Option<String>,
) -> RunSummary {
    let reduction_pct = ratio_pct(stats.duplicates, stats.tokens_seen);
    let uniq_pct = ratio_pct(stats.unique_tokens, stats.tokens_seen);
    let elapsed_sec = (stats.elapsed_ms as f64) / 1000.0;
    let avg_throughput_tps = if elapsed_sec > 0.0 {
        ((stats.tokens_seen as f64) / elapsed_sec).round() as u64
    } else {
        0
    };
    let mode = mode_name(config.mode).to_string();
    let mode_effective = mode_effective_name(config.mode).to_string();
    let ordering = ordering_name(config.ordering).to_string();
    let stage_durations_ms = if bridge.stage_durations_ms.is_empty() {
        None
    } else {
        Some(bridge.stage_durations_ms.clone())
    };

    RunSummary {
        job_id,
        status,
        started_at: started_at.to_rfc3339_opts(SecondsFormat::Millis, true),
        finished_at: finished_at.to_rfc3339_opts(SecondsFormat::Millis, true),
        files_total: config.inputs.len(),
        files_done: bridge.snapshot.files_done,
        tokens_seen: stats.tokens_seen,
        unique_tokens: stats.unique_tokens,
        duplicates: stats.duplicates,
        reduction_pct,
        uniq_pct,
        input_bytes_total,
        output_path: config.output.to_string_lossy().to_string(),
        output_bytes,
        mode,
        mode_effective: mode_effective.clone(),
        ordering: ordering.clone(),
        disk_alphabetical_mode: if mode_effective == "disk" {
            Some(disk_mode_name(config.disk_alphabetical_mode).to_string())
        } else {
            None
        },
        disk_buckets: if mode_effective == "disk" {
            Some(config.disk_buckets)
        } else {
            None
        },
        disk_run_bytes: if mode_effective == "disk" {
            Some(config.disk_run_bytes)
        } else {
            None
        },
        trim: config.trim,
        drop_empty: config.drop_empty,
        output_separator_raw: escape_separator_raw(&config.output_separator),
        output_separator_preview: separator_preview(&config.output_separator),
        elapsed_ms: stats.elapsed_ms,
        avg_throughput_tps,
        peak_throughput_tps: if bridge.peak_throughput_tps > 0 {
            Some(bridge.peak_throughput_tps)
        } else {
            None
        },
        stage_durations_ms,
        temp_bytes_total: None,
        warnings: build_warnings(config, status, reduction_pct, uniq_pct),
        error_message,
    }
}

fn build_warnings(
    config: &Config,
    status: RunTerminalStatus,
    reduction_pct: f64,
    uniq_pct: f64,
) -> Vec<String> {
    let mut out = Vec::new();

    if matches!(config.ordering, OutputOrdering::UnorderedFast) {
        out.push("UnorderedFast: output order is not guaranteed.".to_string());
    }

    if matches!(config.mode, Mode::Disk)
        && matches!(config.ordering, OutputOrdering::PreserveFirstSeen)
    {
        out.push("DISK + PreserveFirstSeen: global order is not guaranteed.".to_string());
    }

    if config.output_separator.chars().count() > 1 {
        out.push(format!(
            "Output separator uses multiple characters: {}",
            escape_separator_raw(&config.output_separator)
        ));
    }

    if reduction_pct >= 80.0 {
        out.push("Very high duplicate rate detected (>80%).".to_string());
    }

    if uniq_pct <= 20.0 {
        out.push("Very low unique rate detected (<=20%).".to_string());
    }

    match status {
        RunTerminalStatus::Error => {
            out.push("Run failed before completion. Output may be incomplete.".to_string());
        }
        RunTerminalStatus::Canceled => {
            out.push("Run was canceled by user. Output may be incomplete.".to_string());
        }
        RunTerminalStatus::Success => {}
    }

    out
}

fn sum_existing_input_bytes(config: &Config) -> u64 {
    config.inputs.iter().map(file_len_or_zero).sum()
}

fn file_len_or_zero(path: impl AsRef<Path>) -> u64 {
    fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

fn ratio_pct(part: u64, total: u64) -> f64 {
    if total == 0 {
        return 0.0;
    }
    let raw = (part as f64 * 100.0) / (total as f64);
    (raw * 100.0).round() / 100.0
}

fn mode_name(mode: Mode) -> &'static str {
    match mode {
        Mode::Auto => "auto",
        Mode::Ram => "ram",
        Mode::Disk => "disk",
    }
}

fn mode_effective_name(mode: Mode) -> &'static str {
    match mode {
        Mode::Auto => "ram",
        Mode::Ram => "ram",
        Mode::Disk => "disk",
    }
}

fn ordering_name(ordering: OutputOrdering) -> &'static str {
    match ordering {
        OutputOrdering::PreserveFirstSeen => "preserve_first_seen",
        OutputOrdering::Alphabetical => "alphabetical",
        OutputOrdering::UnorderedFast => "unordered_fast",
    }
}

fn disk_mode_name(mode: DiskAlphabeticalMode) -> &'static str {
    match mode {
        DiskAlphabeticalMode::FastBucketLocal => "fast_bucket_local",
        DiskAlphabeticalMode::GlobalPerfect => "global_perfect",
    }
}

fn escape_separator_raw(separator: &str) -> String {
    let mut out = String::new();
    for c in separator.chars() {
        match c {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{000C}' => out.push_str("\\f"),
            '\\' => out.push_str("\\\\"),
            _ => out.push(c),
        }
    }
    out
}

fn separator_preview(separator: &str) -> String {
    let mut out = String::new();
    for c in separator.chars() {
        match c {
            '\n' => out.push('↵'),
            '\r' => out.push('␍'),
            '\t' => out.push('⇥'),
            '\u{000C}' => out.push('␌'),
            _ => out.push(c),
        }
    }
    out
}
