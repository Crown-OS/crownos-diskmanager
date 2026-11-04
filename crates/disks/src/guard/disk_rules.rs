use super::system::{DiskRole, disk_role};
use super::{Action, Refusal};
use crate::model::{Disk, DiskKind, Region, Removal};

pub(super) fn check(disk: &Disk, action: Action) -> Result<(), Refusal> {
    let role = disk_role(disk);
    match action {
        Action::CreateImage => Ok(()),
        Action::Eject => protect(role).and_then(|()| ejectable(disk)),
        Action::EraseDisk | Action::RestoreImage => protect(role).and_then(|()| rewritable(disk)),
        Action::CreatePartition => refuse_live_medium(role).and_then(|()| has_free_space(disk)),
        Action::Mount
        | Action::Unmount
        | Action::Rename
        | Action::Unlock
        | Action::Lock
        | Action::Check
        | Action::Repair
        | Action::Format
        | Action::DeletePartition
        | Action::Resize => Err(Refusal::NotApplicable),
    }
}

pub(super) fn protect(role: DiskRole) -> Result<(), Refusal> {
    match role {
        DiskRole::Ordinary => Ok(()),
        DiskRole::System => Err(Refusal::SystemDisk),
        DiskRole::LiveMedium => Err(Refusal::LiveMedium),
    }
}

pub(super) fn refuse_live_medium(role: DiskRole) -> Result<(), Refusal> {
    match role {
        DiskRole::LiveMedium => Err(Refusal::LiveMedium),
        DiskRole::Ordinary | DiskRole::System => Ok(()),
    }
}

fn ejectable(disk: &Disk) -> Result<(), Refusal> {
    let removable = matches!(disk.removal, Removal::Removable { .. });
    match removable || disk.kind.is_external() || disk.kind == DiskKind::Image {
        true => Ok(()),
        false => Err(Refusal::NotApplicable),
    }
}

fn rewritable(disk: &Disk) -> Result<(), Refusal> {
    match disk.kind {
        DiskKind::Optical => Err(Refusal::NotApplicable),
        _ => Ok(()),
    }
}

fn has_free_space(disk: &Disk) -> Result<(), Refusal> {
    if disk.table.is_none() {
        return Err(Refusal::NoTable);
    }
    match disk
        .layout
        .iter()
        .any(|region| matches!(region, Region::Free { .. }))
    {
        true => Ok(()),
        false => Err(Refusal::NotApplicable),
    }
}
