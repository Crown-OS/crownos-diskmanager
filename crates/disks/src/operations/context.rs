use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::command::{Event, OperationId, Progress};
use crate::error::StorageError;

pub(crate) type EventSink = Arc<dyn Fn(Event) + Send + Sync>;

#[derive(Clone)]
pub(crate) struct OperationContext {
    pub id: OperationId,
    sink: EventSink,
    cancelled: Arc<AtomicBool>,
}

impl OperationContext {
    pub fn new(id: OperationId, sink: EventSink) -> Self {
        Self {
            id,
            sink,
            cancelled: Arc::default(),
        }
    }

    pub fn report(&self, phase: &str, progress: Progress) {
        (self.sink)(Event::OperationProgress {
            id: self.id,
            phase: phase.to_owned(),
            progress,
        });
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    pub fn ensure_not_cancelled(&self) -> Result<(), StorageError> {
        match self.cancelled.load(Ordering::Relaxed) {
            true => Err(StorageError::Cancelled),
            false => Ok(()),
        }
    }
}
