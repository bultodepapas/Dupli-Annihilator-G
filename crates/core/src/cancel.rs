use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

#[derive(Debug, thiserror::Error)]
#[error("job canceled")]
pub struct Canceled;

pub fn is_canceled_error(err: &anyhow::Error) -> bool {
    err.downcast_ref::<Canceled>().is_some()
}

pub trait CancelCheck: Send + Sync + 'static {
    fn is_canceled(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NoCancel;

impl CancelCheck for NoCancel {}

#[derive(Debug, Clone, Default)]
pub struct CancellationToken {
    flag: Arc<AtomicBool>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self {
            flag: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn cancel(&self) {
        self.flag.store(true, Ordering::Release);
    }

    pub fn is_canceled(&self) -> bool {
        self.flag.load(Ordering::Acquire)
    }
}

impl CancelCheck for CancellationToken {
    fn is_canceled(&self) -> bool {
        self.flag.load(Ordering::Acquire)
    }
}

#[inline]
pub(crate) fn ensure_not_canceled<C: CancelCheck>(cancel: &C) -> anyhow::Result<()> {
    if cancel.is_canceled() {
        return Err(Canceled.into());
    }
    Ok(())
}
