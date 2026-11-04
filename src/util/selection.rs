use crownos_disks::command::Target;
use crownos_disks::model::{DiskId, Inventory, Region};

use crate::state::Selection;

pub fn selected_disk(selection: &Selection) -> Option<&DiskId> {
    match selection {
        Selection::Overview => None,
        Selection::Disk(disk)
        | Selection::Partition { disk, .. }
        | Selection::Free { disk, .. } => Some(disk),
    }
}

pub fn selection_target(selection: &Selection) -> Option<Target> {
    match selection {
        Selection::Overview => None,
        Selection::Disk(disk) | Selection::Free { disk, .. } => Some(Target::Disk(disk.clone())),
        Selection::Partition { block, .. } => Some(Target::Block(block.clone())),
    }
}

pub fn region_selection(disk: &DiskId, region: &Region) -> Selection {
    match region {
        Region::Partition(partition) => Selection::Partition {
            disk: disk.clone(),
            block: partition.block.clone(),
        },
        Region::Free { offset, .. } => Selection::Free {
            disk: disk.clone(),
            offset: *offset,
        },
    }
}

pub fn still_present(inventory: &Inventory, selection: &Selection) -> bool {
    match selection {
        Selection::Overview => true,
        Selection::Disk(disk) => inventory.disk(disk).is_some(),
        Selection::Partition { disk, block } => inventory
            .disk(disk)
            .is_some_and(|found| found.partition(block).is_some()),
        Selection::Free { disk, offset } => inventory.disk(disk).is_some_and(|found| {
            found
                .layout
                .iter()
                .any(|region| matches!(region, Region::Free { offset: free, .. } if free == offset))
        }),
    }
}
