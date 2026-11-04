use super::disk_rules::refuse_live_medium;
use super::system::{DiskRole, disk_role, in_system_use};
use super::target::BlockTarget;
use super::{Action, Refusal};
use crate::limits;
use crate::model::{Content, FilesystemKind, Inventory, Volume};

const CHECKABLE: [FilesystemKind; 9] = [
    FilesystemKind::Ext4,
    FilesystemKind::Ext3,
    FilesystemKind::Ext2,
    FilesystemKind::Xfs,
    FilesystemKind::Btrfs,
    FilesystemKind::Fat32,
    FilesystemKind::Exfat,
    FilesystemKind::Ntfs,
    FilesystemKind::F2fs,
];

pub(super) fn check(
    inventory: &Inventory,
    target: &BlockTarget<'_>,
    action: Action,
) -> Result<(), Refusal> {
    let role = disk_role(target.disk);
    match action {
        Action::Mount => mount(target),
        Action::CreateImage => Ok(()),
        Action::Unmount => refuse_live_medium(role).and_then(|()| unmount(target)),
        Action::Rename => refuse_live_medium(role).and_then(|()| rename(target)),
        Action::Unlock => unlock(target),
        Action::Lock => lock(target),
        Action::Check | Action::Repair => {
            refuse_live_medium(role).and_then(|()| check_filesystem(target))
        }
        Action::Format | Action::RestoreImage => replace(target, role),
        Action::DeletePartition => delete(target, role),
        Action::Resize => limits::resize_bounds(inventory, target.block()).map(drop),
        Action::Eject | Action::EraseDisk | Action::CreatePartition => Err(Refusal::NotApplicable),
    }
}

fn mounted_volume<'a>(target: &BlockTarget<'a>) -> Result<&'a Volume, Refusal> {
    match target.volume() {
        Some(volume) => Ok(volume),
        None if target.is_locked() => Err(Refusal::Locked {
            name: target.name(),
        }),
        None => Err(Refusal::NotApplicable),
    }
}

fn busy(target: &BlockTarget<'_>) -> Refusal {
    match in_system_use(target.content()) {
        true => Refusal::SystemVolume {
            name: target.name(),
        },
        false => Refusal::Mounted {
            name: target.name(),
        },
    }
}

fn mount(target: &BlockTarget<'_>) -> Result<(), Refusal> {
    match mounted_volume(target)?.is_mounted() {
        true => Err(Refusal::NotApplicable),
        false => Ok(()),
    }
}

fn unmount(target: &BlockTarget<'_>) -> Result<(), Refusal> {
    if !mounted_volume(target)?.is_mounted() {
        return Err(Refusal::NotApplicable);
    }
    match in_system_use(target.content()) {
        true => Err(Refusal::SystemVolume {
            name: target.name(),
        }),
        false => Ok(()),
    }
}

fn rename(target: &BlockTarget<'_>) -> Result<(), Refusal> {
    if matches!(target.content(), Content::Swap { .. }) {
        return Ok(());
    }
    let volume = mounted_volume(target)?;
    if let FilesystemKind::Other(name) = &volume.filesystem {
        return Err(Refusal::Unsupported {
            filesystem: name.to_string(),
        });
    }
    match volume.is_mounted() && limits::relabel_requires_unmount(&volume.filesystem) {
        true => Err(busy(target)),
        false => Ok(()),
    }
}

fn unlock(target: &BlockTarget<'_>) -> Result<(), Refusal> {
    match target.is_locked() {
        true => Ok(()),
        false => Err(Refusal::NotApplicable),
    }
}

fn lock(target: &BlockTarget<'_>) -> Result<(), Refusal> {
    let unlocked = target
        .encryption()
        .is_some_and(|encryption| encryption.unlocked.is_some());
    if !unlocked {
        return Err(Refusal::NotApplicable);
    }
    match in_system_use(&target.partition.content) {
        true => Err(Refusal::SystemVolume {
            name: target.name(),
        }),
        false => Ok(()),
    }
}

fn check_filesystem(target: &BlockTarget<'_>) -> Result<(), Refusal> {
    let volume = mounted_volume(target)?;
    if !CHECKABLE.contains(&volume.filesystem) {
        return Err(Refusal::Unsupported {
            filesystem: volume.filesystem.short_name().to_owned(),
        });
    }
    match volume.is_mounted() {
        true => Err(busy(target)),
        false => Ok(()),
    }
}

fn replace(target: &BlockTarget<'_>, role: DiskRole) -> Result<(), Refusal> {
    refuse_live_medium(role)?;
    if target.is_extended_container() {
        return Err(Refusal::NotApplicable);
    }
    match in_system_use(&target.partition.content) {
        true => Err(Refusal::SystemVolume {
            name: target.name(),
        }),
        false => Ok(()),
    }
}

fn delete(target: &BlockTarget<'_>, role: DiskRole) -> Result<(), Refusal> {
    match target.is_whole_disk() {
        true => Err(Refusal::NotApplicable),
        false => replace(target, role),
    }
}
