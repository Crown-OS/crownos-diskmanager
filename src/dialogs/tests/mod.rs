mod completion;
mod confirm;
mod forms;
mod harness;
mod progress;

use crownos_disks::model::{BlockId, DiskId, Inventory};

fn disk_named(inventory: &Inventory, name: &str) -> DiskId {
    inventory
        .disks
        .iter()
        .find(|disk| disk.name == name)
        .map(|disk| disk.id.clone())
        .unwrap_or_else(|| panic!("no disk named {name}"))
}

fn block_titled(inventory: &Inventory, title: &str) -> BlockId {
    inventory
        .disks
        .iter()
        .flat_map(|disk| disk.partitions())
        .find(|partition| crate::util::region_text::partition_title(partition) == title)
        .map(|partition| partition.block.clone())
        .unwrap_or_else(|| panic!("no partition titled {title}"))
}
