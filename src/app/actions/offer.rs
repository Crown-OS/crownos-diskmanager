use crownos_disks::guard::{self, Availability, Refusal};
use crownos_disks::model::{Content, Disk, DiskKind, Encryption, Inventory, Partition, Removal};

use super::action::DiskAction;
use crate::state::{Operations, Selection};
use crate::util::selection::{selected_disk, selection_target};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    pub action: DiskAction,
    pub selection: Selection,
    pub refusal: Option<Refusal>,
}

pub fn offers(inventory: &Inventory, operations: &Operations, selection: &Selection) -> Vec<Offer> {
    let Some(target) = selection_target(selection) else {
        return Vec::new();
    };
    let busy = selected_disk(selection).is_some_and(|disk| operations.is_busy(disk, inventory));
    contextual_actions(inventory, selection)
        .into_iter()
        .map(|action| {
            let refusal = match busy {
                true => Some(Refusal::Busy),
                false => action.guard_action().and_then(|guarded| {
                    match guard::availability(inventory, &target, guarded) {
                        Availability::Available => None,
                        Availability::Unavailable(refusal) => Some(refusal),
                    }
                }),
            };
            Offer {
                action,
                selection: selection.clone(),
                refusal,
            }
        })
        .collect()
}

fn contextual_actions(inventory: &Inventory, selection: &Selection) -> Vec<DiskAction> {
    match selection {
        Selection::Overview => Vec::new(),
        Selection::Disk(disk) => inventory.disk(disk).map(disk_actions).unwrap_or_default(),
        Selection::Partition { block, .. } => inventory
            .partition(block)
            .map(|(_, partition)| volume_actions(partition))
            .unwrap_or_default(),
        Selection::Free { .. } => vec![DiskAction::NewPartition],
    }
}

fn disk_actions(disk: &Disk) -> Vec<DiskAction> {
    let mut actions = vec![
        DiskAction::Erase,
        DiskAction::CreateImage,
        DiskAction::RestoreImage,
    ];
    match (disk.kind, disk.removal) {
        (DiskKind::Image, _) => actions.push(DiskAction::CloseImage),
        (_, Removal::Removable { .. }) => actions.push(DiskAction::Eject),
        (_, Removal::Fixed) => {}
    }
    actions
}

fn volume_actions(partition: &Partition) -> Vec<DiskAction> {
    use DiskAction::{
        CheckRepair, CreateImage, Delete, Format, Lock, Mount, Rename, Resize, RestoreImage,
        Unlock, Unmount,
    };
    let mount_toggle = |content: &Content| match content.volume() {
        Some(volume) if volume.is_mounted() => Unmount,
        _ => Mount,
    };
    match &partition.content {
        Content::Filesystem(_) => vec![
            mount_toggle(&partition.content),
            CheckRepair,
            Resize,
            Format,
            CreateImage,
            RestoreImage,
            Rename,
            Delete,
        ],
        Content::Encrypted(Encryption { unlocked: None, .. }) => {
            vec![Unlock, Format, CreateImage, RestoreImage, Delete]
        }
        Content::Encrypted(Encryption {
            unlocked: Some(_), ..
        }) => vec![
            mount_toggle(&partition.content),
            Lock,
            CheckRepair,
            Resize,
            Format,
            CreateImage,
            RestoreImage,
            Rename,
            Delete,
        ],
        Content::Swap { .. }
        | Content::Member { .. }
        | Content::Unknown { .. }
        | Content::Empty => {
            vec![Format, CreateImage, RestoreImage, Delete]
        }
    }
}
