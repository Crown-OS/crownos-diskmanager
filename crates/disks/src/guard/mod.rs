mod block_rules;
mod disk_rules;
mod fingerprint;
#[cfg(test)]
mod fixture_tests;
mod losses;
mod system;
pub(crate) mod target;
#[cfg(test)]
pub(crate) mod test_inventory;
#[cfg(test)]
mod tests;
mod tier;

use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::command::Target;
use crate::model::{ByteSize, FilesystemKind, Inventory};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    Mount,
    Unmount,
    Eject,
    Rename,
    Unlock,
    Lock,
    Check,
    Repair,
    Format,
    EraseDisk,
    CreatePartition,
    DeletePartition,
    Resize,
    CreateImage,
    RestoreImage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Availability {
    Available,
    Unavailable(Refusal),
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Refusal {
    #[error("This is the disk CrownOS is running from.")]
    SystemDisk,
    #[error("This is the installation medium CrownOS was started from.")]
    LiveMedium,
    #[error("{name} is in use by the system.")]
    SystemVolume { name: String },
    #[error("Unmount {name} first.")]
    Mounted { name: String },
    #[error("{name} must be mounted to do this.")]
    NotMounted { name: String },
    #[error("Unlock {name} first.")]
    Locked { name: String },
    #[error("{filesystem} doesn't support this.")]
    Unsupported { filesystem: String },
    #[error("Formatting as {filesystem} requires the {tool} package.")]
    MissingTool { filesystem: String, tool: String },
    #[error("Another operation is running on this disk.")]
    Busy,
    #[error("There is no free space after this partition.")]
    NoRoom,
    #[error("This disk has no partition table.")]
    NoTable,
    #[error("This isn't possible here.")]
    NotApplicable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConfirmationTier {
    None,
    Simple,
    Destructive,
    TypeName,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fingerprint {
    pub drive_serial: String,
    pub size: ByteSize,
    pub table_uuid: String,
    pub partitions: Vec<(String, ByteSize)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Loss {
    pub name: String,
    pub filesystem: Option<FilesystemKind>,
    pub used: Option<ByteSize>,
}

impl fmt::Display for Loss {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.name)
    }
}

pub(crate) use fingerprint::of_disk as disk_fingerprint;
pub(crate) use system::{DiskRole, disk_role, in_system_use};

/// Whether `action` can run on `target` right now, and why not when it can't.
pub fn availability(inventory: &Inventory, target: &Target, action: Action) -> Availability {
    let verdict = match target::resolve(inventory, target) {
        None => Err(Refusal::NotApplicable),
        Some(target::Resolved::Disk(disk)) => disk_rules::check(disk, action),
        Some(target::Resolved::Block(block)) => block_rules::check(inventory, &block, action),
    };
    verdict.map_or_else(Availability::Unavailable, |()| Availability::Available)
}

/// How strongly the user must confirm `action` on `target`.
///
/// Resize reports the shrink tier; use [`resize_confirmation_tier`] once the size is chosen.
pub fn confirmation_tier(
    inventory: &Inventory,
    target: &Target,
    action: Action,
) -> ConfirmationTier {
    tier::confirmation_tier(inventory, target, action)
}

/// Shrinking asks for a simple confirmation, growing needs none.
pub fn resize_confirmation_tier(current: ByteSize, requested: ByteSize) -> ConfirmationTier {
    tier::resize_confirmation_tier(current, requested)
}

/// Identity of the disk holding `target`, captured when a confirmation opens.
pub fn fingerprint(inventory: &Inventory, target: &Target) -> Option<Fingerprint> {
    fingerprint::fingerprint(inventory, target)
}

/// Every volume whose contents a destructive action on `target` destroys.
pub fn losses(inventory: &Inventory, target: &Target) -> Vec<Loss> {
    losses::losses(inventory, target)
}

impl Availability {
    pub const fn is_available(&self) -> bool {
        matches!(self, Self::Available)
    }
}
