#[derive(Debug, Clone)]
pub enum ProgressEvent {
    Stage(&'static str),
    FileStarted { index: usize, total: usize },
    FileFinished { index: usize, total: usize },
    TokensSeen(u64),
    UniqueTokens(u64),
    Duplicates(u64),
}

pub trait ProgressSink: Send + Sync + 'static {
    fn on_event(&self, _event: ProgressEvent) {}
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoProgress;

impl ProgressSink for NoProgress {}
