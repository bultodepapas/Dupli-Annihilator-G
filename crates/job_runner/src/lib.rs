use anyhow::anyhow;
use dedupe_core::{
    is_canceled_error, run_with_control, CancellationToken, Config, ProgressEvent, ProgressSink,
    Stats,
};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc::{self, Receiver, Sender},
    Arc, Mutex,
};
use std::time::Duration;

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

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
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
        tokens_seen: u64,
        unique_tokens: u64,
        duplicates: u64,
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
        let active_slot = Arc::clone(&self.inner.active);
        std::thread::spawn(move || {
            let _ = tx.send(JobEvent::Started { job_id });

            let sink = BridgeSink::new(job_id, tx.clone());
            let result = run_with_control(&config, sink, cancel.clone());

            match result {
                Ok(stats) => {
                    let _ = tx.send(JobEvent::Done {
                        job_id,
                        stats: StatsSnapshot::from_stats(stats),
                    });
                }
                Err(err) => {
                    if is_canceled_error(&err) || cancel.is_canceled() {
                        let _ = tx.send(JobEvent::Canceled { job_id });
                    } else {
                        let _ = tx.send(JobEvent::Error {
                            job_id,
                            message: format!("{err:#}"),
                        });
                    }
                }
            }

            done.store(true, Ordering::Release);
            if let Ok(mut current) = active_slot.lock() {
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
    tokens_seen: u64,
    unique_tokens: u64,
    duplicates: u64,
}

#[derive(Debug)]
struct BridgeSink {
    job_id: JobId,
    tx: Sender<JobEvent>,
    snapshot: Mutex<ProgressSnapshot>,
}

impl BridgeSink {
    fn new(job_id: JobId, tx: Sender<JobEvent>) -> Self {
        Self {
            job_id,
            tx,
            snapshot: Mutex::new(ProgressSnapshot::default()),
        }
    }

    fn emit_progress(&self, snapshot: &ProgressSnapshot) {
        let _ = self.tx.send(JobEvent::Progress {
            job_id: self.job_id,
            stage: snapshot.stage.clone(),
            files_done: snapshot.files_done,
            files_total: snapshot.files_total,
            tokens_seen: snapshot.tokens_seen,
            unique_tokens: snapshot.unique_tokens,
            duplicates: snapshot.duplicates,
        });
    }
}

impl ProgressSink for BridgeSink {
    fn on_event(&self, event: ProgressEvent) {
        let mut snapshot = match self.snapshot.lock() {
            Ok(lock) => lock,
            Err(_) => return,
        };

        match event {
            ProgressEvent::Stage(stage) => {
                snapshot.stage = Some(stage.to_string());
                let _ = self.tx.send(JobEvent::Stage {
                    job_id: self.job_id,
                    stage: stage.to_string(),
                });
            }
            ProgressEvent::FileStarted { index: _, total } => {
                snapshot.files_total = total;
            }
            ProgressEvent::FileFinished { index, total } => {
                snapshot.files_done = index;
                snapshot.files_total = total;
            }
            ProgressEvent::TokensSeen(v) => {
                snapshot.tokens_seen = v;
            }
            ProgressEvent::UniqueTokens(v) => {
                snapshot.unique_tokens = v;
            }
            ProgressEvent::Duplicates(v) => {
                snapshot.duplicates = v;
            }
        }

        let snapshot_copy = snapshot.clone();
        drop(snapshot);
        self.emit_progress(&snapshot_copy);
    }
}
