use super::target::{Resolved, resolve};
use super::{Action, ConfirmationTier};
use crate::command::Target;
use crate::model::{ByteSize, Disk, DiskKind, Inventory, Removal};

pub(super) fn confirmation_tier(
    inventory: &Inventory,
    target: &Target,
    action: Action,
) -> ConfirmationTier {
    match action {
        Action::Mount
        | Action::Unmount
        | Action::Rename
        | Action::Eject
        | Action::Unlock
        | Action::Lock
        | Action::CreateImage => ConfirmationTier::None,
        Action::CreatePartition | Action::Resize | Action::Check | Action::Repair => {
            ConfirmationTier::Simple
        }
        Action::Format | Action::DeletePartition => ConfirmationTier::Destructive,
        Action::EraseDisk | Action::RestoreImage => match resolve(inventory, target) {
            Some(Resolved::Disk(disk)) if is_internal(disk) => ConfirmationTier::TypeName,
            _ => ConfirmationTier::Destructive,
        },
    }
}

pub(super) fn resize_confirmation_tier(current: ByteSize, requested: ByteSize) -> ConfirmationTier {
    match requested < current {
        true => ConfirmationTier::Simple,
        false => ConfirmationTier::None,
    }
}

fn is_internal(disk: &Disk) -> bool {
    disk.removal == Removal::Fixed && !disk.kind.is_external() && disk.kind != DiskKind::Image
}
