mod default_filesystem;
mod label;
pub(crate) mod resize;
#[cfg(test)]
mod resize_tests;

use thiserror::Error;

use crate::guard::Refusal;
use crate::model::{BlockId, ByteSize, Disk, FilesystemKind, Inventory};

pub(crate) use label::relabel_requires_unmount;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResizeBounds {
    pub min: ByteSize,
    pub current: ByteSize,
    pub max: ByteSize,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LabelError {
    #[error("Names for {filesystem} can be at most {limit} characters.")]
    TooLong { filesystem: String, limit: usize },
    #[error("Names for {filesystem} can't contain “{character}”.")]
    InvalidCharacter { filesystem: String, character: char },
}

/// The partition sizes `block` can be resized to, given its contents and the free space after it.
pub fn resize_bounds(inventory: &Inventory, block: &BlockId) -> Result<ResizeBounds, Refusal> {
    resize::plan(inventory, block).map(|plan| plan.bounds)
}

pub fn validate_label(filesystem: &FilesystemKind, label: &str) -> Result<(), LabelError> {
    label::validate_label(filesystem, label)
}

/// The filesystem suggested when formatting `disk`.
pub fn default_filesystem(disk: &Disk) -> FilesystemKind {
    default_filesystem::default_filesystem(disk)
}
