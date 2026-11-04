use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::StorageError;
use crate::guard::disk_fingerprint;
use crate::journal::{Journal, JournalEntry, JournalOperation, TableSnapshot};
use crate::model::{BlockId, Disk};
use crate::util::blocking::run_blocking;

pub(crate) fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

pub(crate) fn entry(
    operation: JournalOperation,
    disk: &Disk,
    block: Option<&BlockId>,
    target_name: String,
) -> JournalEntry {
    JournalEntry {
        operation,
        disk: disk.id.clone(),
        block: block.cloned(),
        target_name,
        fingerprint: disk_fingerprint(disk),
        completed_steps: 0,
        started_at_unix: unix_now(),
    }
}

/// Keeps the on-disk journal entry of one running operation in step with its progress.
pub(crate) struct Recorder {
    journal: Option<Journal>,
    entry: JournalEntry,
}

impl Recorder {
    /// Writes `entry` before the operation touches the disk.
    pub async fn begin(
        journal: Option<&Journal>,
        entry: JournalEntry,
    ) -> Result<Self, StorageError> {
        let recorder = Self {
            journal: journal.cloned(),
            entry,
        };
        recorder.write().await?;
        Ok(recorder)
    }

    /// Continues an entry left behind by an interrupted run.
    pub fn resume(journal: Option<&Journal>, entry: JournalEntry) -> Self {
        Self {
            journal: journal.cloned(),
            entry,
        }
    }

    pub fn completed_steps(&self) -> u8 {
        self.entry.completed_steps
    }

    pub async fn advance(&mut self) -> Result<(), StorageError> {
        self.entry.completed_steps = self.entry.completed_steps.saturating_add(1);
        self.write().await
    }

    /// Removes the entry once the disk is consistent again.
    pub async fn finish(self) -> Result<(), StorageError> {
        let Some(journal) = self.journal else {
            return Ok(());
        };
        let entry = self.entry;
        run_blocking(move || Ok(journal.finish(&entry)?)).await
    }

    async fn write(&self) -> Result<(), StorageError> {
        let Some(journal) = self.journal.clone() else {
            return Ok(());
        };
        let entry = self.entry.clone();
        run_blocking(move || Ok(journal.begin(&entry)?)).await
    }
}

/// Saves the disk's partition layout before its table changes.
pub(crate) async fn save_table(journal: Option<&Journal>, disk: &Disk) -> Result<(), StorageError> {
    let Some(journal) = journal.cloned() else {
        return Ok(());
    };
    let snapshot = TableSnapshot::of(disk, unix_now());
    run_blocking(move || {
        journal
            .save_table(&snapshot)
            .map(drop)
            .map_err(StorageError::from)
    })
    .await
}
