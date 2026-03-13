use crate::config::Mode;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum ProgressEvent {
    Stage(&'static str),
    FileStarted {
        index: usize,
        total: usize,
    },
    FileFinished {
        index: usize,
        total: usize,
    },
    StageItemStarted {
        total: usize,
        path: PathBuf,
    },
    StageItemFinished {
        completed: usize,
        total: usize,
        path: PathBuf,
    },
    TokensSeen(u64),
    UniqueTokens(u64),
    Duplicates(u64),
    /// Running total of tokens dropped by the length filter.
    FilteredByLength(u64),
    ModeResolved(Mode),
}

pub trait ProgressSink: Send + Sync + 'static {
    fn on_event(&self, _event: ProgressEvent) {}
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoProgress;

impl ProgressSink for NoProgress {}

/// Blanket impl so `Arc<T: ProgressSink>` can be passed anywhere a
/// `ProgressSink` is expected, without requiring a newtype wrapper.
impl<T: ProgressSink> ProgressSink for std::sync::Arc<T> {
    fn on_event(&self, event: ProgressEvent) {
        T::on_event(self.as_ref(), event);
    }
}
