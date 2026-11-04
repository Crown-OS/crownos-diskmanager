use crate::command::Target;
use crate::error::StorageError;
use crate::model::{BlockId, ByteSize, Disk, Inventory, Partition};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TargetLayout {
    WholeDisk,
    Partition,
}

#[derive(Debug, Clone)]
pub(super) struct ResolvedTarget {
    pub block: BlockId,
    pub name: String,
    pub capacity: ByteSize,
    pub layout: TargetLayout,
}

pub(super) fn resolve(
    inventory: &Inventory,
    target: &Target,
) -> Result<ResolvedTarget, StorageError> {
    match target {
        Target::Disk(id) => inventory.disk(id).map(whole_disk),
        Target::Block(block) => inventory
            .disks
            .iter()
            .find(|disk| &disk.block == block)
            .map(whole_disk)
            .or_else(|| {
                inventory
                    .partition(block)
                    .map(|(disk, partition)| partition_of(disk, partition))
            }),
    }
    .ok_or_else(|| StorageError::Gone(String::from("The disk")))
}

pub(super) fn disk_label(disk: &Disk) -> String {
    format!("{} ({})", disk.name, disk.size)
}

fn whole_disk(disk: &Disk) -> ResolvedTarget {
    ResolvedTarget {
        block: disk.block.clone(),
        name: disk_label(disk),
        capacity: disk.size,
        layout: TargetLayout::WholeDisk,
    }
}

fn partition_of(disk: &Disk, partition: &Partition) -> ResolvedTarget {
    let name = partition
        .content
        .volume()
        .map(|volume| volume.label.as_str())
        .filter(|label| !label.is_empty())
        .map_or_else(
            || format!("{} partition {}", disk.name, partition.number),
            str::to_owned,
        );
    ResolvedTarget {
        block: partition.block.clone(),
        name,
        capacity: partition.size,
        layout: TargetLayout::Partition,
    }
}
