use crownos_disks::Command;
use crownos_disks::command::Target;
use crownos_disks::model::{ByteSize, DiskId, Inventory, Region};
use crownui::prelude::Cx;

use super::action::DiskAction;
use crate::state::{DialogRequest, Selection, Store};
use crate::util::selection::selection_target;

enum Step {
    Send(Command),
    Open(DialogRequest),
}

pub fn perform(cx: &mut Cx, store: Store, selection: &Selection, action: DiskAction) {
    let step = store
        .inventory
        .with_untracked(cx, |inventory| step_for(inventory, selection, action));
    match step {
        Some(Step::Send(command)) => store.send(cx, command),
        Some(Step::Open(request)) => store.open_dialog(cx, request),
        None => {}
    }
}

fn step_for(inventory: &Inventory, selection: &Selection, action: DiskAction) -> Option<Step> {
    let target = selection_target(selection)?;
    Some(match (action, &target) {
        (DiskAction::Mount, Target::Block(block)) => Step::Send(Command::Mount(block.clone())),
        (DiskAction::Unmount, Target::Block(block)) => Step::Send(Command::Unmount(block.clone())),
        (DiskAction::Lock, Target::Block(block)) => Step::Send(Command::Lock(block.clone())),
        (DiskAction::CheckRepair, Target::Block(block)) => {
            Step::Open(DialogRequest::Repair(block.clone()))
        }
        (DiskAction::Eject, Target::Disk(disk)) => Step::Send(Command::Eject(disk.clone())),
        (DiskAction::CloseImage, Target::Disk(disk)) => {
            Step::Send(Command::CloseImage(disk.clone()))
        }
        (DiskAction::Unlock, Target::Block(block)) => {
            Step::Open(DialogRequest::Unlock(block.clone()))
        }
        (DiskAction::Resize, Target::Block(block)) => {
            Step::Open(DialogRequest::Resize(block.clone()))
        }
        (DiskAction::Format, Target::Block(block)) => {
            Step::Open(DialogRequest::Format(block.clone()))
        }
        (DiskAction::Rename, Target::Block(block)) => {
            Step::Open(DialogRequest::Rename(block.clone()))
        }
        (DiskAction::Delete, Target::Block(block)) => {
            Step::Open(DialogRequest::Delete(block.clone()))
        }
        (DiskAction::Erase, Target::Disk(disk)) => {
            Step::Open(DialogRequest::EraseDisk(disk.clone()))
        }
        (DiskAction::CreateImage, _) => Step::Open(DialogRequest::CreateImage(target)),
        (DiskAction::RestoreImage, _) => Step::Open(DialogRequest::RestoreImage(target)),
        (DiskAction::NewPartition, Target::Disk(disk)) => {
            Step::Open(new_partition(inventory, disk, selection)?)
        }
        _ => return None,
    })
}

fn new_partition(
    inventory: &Inventory,
    disk: &DiskId,
    selection: &Selection,
) -> Option<DialogRequest> {
    let Selection::Free { offset, .. } = selection else {
        return None;
    };
    let size = free_size(inventory, disk, *offset)?;
    Some(DialogRequest::NewPartition {
        disk: disk.clone(),
        offset: *offset,
        size,
    })
}

fn free_size(inventory: &Inventory, disk: &DiskId, offset: ByteSize) -> Option<ByteSize> {
    inventory
        .disk(disk)?
        .layout
        .iter()
        .find_map(|region| match region {
            Region::Free {
                offset: start,
                size,
            } if *start == offset => Some(*size),
            _ => None,
        })
}
