use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::error::UserError;
use crate::guard::Fingerprint;
use crate::journal::JournalEntry;
use crate::model::{BlockId, ByteSize, DiskId, FilesystemKind, Inventory};

#[derive(Clone, Default)]
pub struct Secret(Zeroizing<String>);

impl Secret {
    pub fn new(value: String) -> Self {
        Self(Zeroizing::new(value))
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Secret(..)")
    }
}

#[derive(Debug, Clone)]
pub struct FormatSpec {
    pub filesystem: FilesystemKind,
    pub label: String,
    pub encryption: Option<Secret>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PartitionScheme {
    Gpt,
    Mbr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Compression {
    None,
    Zstd,
    Xz,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Target {
    Disk(DiskId),
    Block(BlockId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct OperationId(pub u64);

#[derive(Debug, Clone)]
pub enum Command {
    Mount(BlockId),
    Unmount(BlockId),
    Eject(DiskId),
    Rename {
        block: BlockId,
        label: String,
    },
    Unlock {
        block: BlockId,
        passphrase: Secret,
    },
    Lock(BlockId),
    Check(BlockId),
    Repair(BlockId),
    Format {
        block: BlockId,
        spec: FormatSpec,
        expected: Fingerprint,
    },
    EraseDisk {
        disk: DiskId,
        scheme: PartitionScheme,
        spec: FormatSpec,
        expected: Fingerprint,
    },
    CreatePartition {
        disk: DiskId,
        offset: ByteSize,
        size: ByteSize,
        name: String,
        spec: FormatSpec,
        expected: Fingerprint,
    },
    DeletePartition {
        block: BlockId,
        expected: Fingerprint,
    },
    Resize {
        block: BlockId,
        size: ByteSize,
        passphrase: Option<Secret>,
        expected: Fingerprint,
    },
    ResumeInterrupted(JournalEntry),
    DismissInterrupted(JournalEntry),
    CreateImage {
        source: Target,
        destination: PathBuf,
        compression: Compression,
    },
    RestoreImage {
        image: PathBuf,
        target: Target,
        verify: bool,
        expected: Fingerprint,
    },
    OpenImage(PathBuf),
    CloseImage(DiskId),
    RefreshHealth(DiskId),
    RefreshUsage,
    Cancel(OperationId),
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Progress {
    pub fraction: Option<f32>,
    pub bytes_per_second: Option<u64>,
    pub seconds_remaining: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct Completion {
    pub message: String,
}

/// What quitting the app does to an operation that is still running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuitImpact {
    /// Over in a moment.
    Harmless,
    /// A single UDisks job, which udisksd finishes after the app exits.
    FinishesInBackground,
    /// Several UDisks jobs run one after another by the app; quitting stops between them.
    StopsPartway,
    /// The app copies the data itself, so quitting loses the image being written.
    LosesImage,
    /// The app copies the data itself, so quitting leaves the target half written.
    LeavesTargetUnusable,
}

impl QuitImpact {
    /// Whether the operation dies with the app and has to be cancelled before quitting.
    pub const fn stops_with_app(self) -> bool {
        matches!(self, Self::LosesImage | Self::LeavesTargetUnusable)
    }
}

#[derive(Debug, Clone)]
pub enum Event {
    Inventory(Arc<Inventory>),
    OperationStarted {
        id: OperationId,
        label: String,
        target: Target,
        cancelable: bool,
        quit_impact: QuitImpact,
    },
    OperationProgress {
        id: OperationId,
        phase: String,
        progress: Progress,
    },
    OperationFinished {
        id: OperationId,
        result: Result<Completion, UserError>,
    },
    Interrupted(Vec<JournalEntry>),
    ServiceUnavailable(UserError),
}
