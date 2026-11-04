mod store;
#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};

use crate::guard::Fingerprint;
use crate::model::{BlockId, ByteSize, Disk, DiskId, TableKind};

pub use store::Journal;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum JournalOperation {
    Shrink { target_size: ByteSize },
    Grow { target_size: ByteSize },
    Format,
    EraseDisk,
    RestoreImage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalEntry {
    pub operation: JournalOperation,
    pub disk: DiskId,
    pub block: Option<BlockId>,
    pub target_name: String,
    pub fingerprint: Fingerprint,
    pub completed_steps: u8,
    pub started_at_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartitionSnapshot {
    pub offset: ByteSize,
    pub size: ByteSize,
    pub type_id: String,
    pub uuid: String,
    pub name: String,
}

/// The partition layout of a disk before a table change, enough to recreate any partition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableSnapshot {
    pub disk_serial: String,
    pub disk_size: ByteSize,
    pub table: Option<TableKind>,
    pub table_uuid: String,
    pub partitions: Vec<PartitionSnapshot>,
    pub taken_at_unix: u64,
}

impl TableSnapshot {
    pub fn of(disk: &Disk, taken_at_unix: u64) -> Self {
        Self {
            disk_serial: crate::guard::disk_fingerprint(disk).drive_serial,
            disk_size: disk.size,
            table: disk.table,
            table_uuid: disk.table_uuid.clone(),
            partitions: disk.partitions().map(PartitionSnapshot::of).collect(),
            taken_at_unix,
        }
    }
}

impl PartitionSnapshot {
    pub fn of(partition: &crate::model::Partition) -> Self {
        Self {
            offset: partition.offset,
            size: partition.size,
            type_id: partition.type_id.clone(),
            uuid: partition.uuid.clone(),
            name: partition.name.clone(),
        }
    }
}

impl JournalEntry {
    pub fn recovery_advice(&self) -> &'static str {
        match self.operation {
            JournalOperation::Shrink { .. } | JournalOperation::Grow { .. } => {
                "Resizing was interrupted. Your data is safe; finish resizing to use the full space."
            }
            JournalOperation::Format => "Formatting was interrupted. Format the volume again.",
            JournalOperation::EraseDisk => "Erasing was interrupted. Erase the disk again.",
            JournalOperation::RestoreImage => {
                "Restoring was interrupted, so the disk's contents are incomplete. Restore the image again or erase the disk."
            }
        }
    }
}
