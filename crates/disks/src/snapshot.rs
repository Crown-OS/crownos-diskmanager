//! `ObjectMap` → `Inventory`, pure and fixture-tested.

mod capabilities;
mod content;
mod disk;
mod health;
mod kind;
mod layout;
mod object_view;
#[cfg(test)]
mod tests;
mod usage;
mod whole_disk;

use std::cmp::Reverse;

pub use layout::WHOLE_DISK_PARTITION_NUMBER;
use object_view::ObjectView;
pub use usage::{fill_usage, usage_mount_points};

use crate::model::{Disk, Inventory};
use crate::object_map::{ObjectMap, interface};

/// Builds the inventory: every drive with media (a whole-disk block of
/// non-zero size, so empty card readers and optical drives are left out), then loop
/// images set up by `uid` (root's loops such as Waydroid's, zram and blocks
/// UDisks asks to ignore are hidden). Usage is left empty; see [`fill_usage`].
pub fn build(map: &ObjectMap, uid: u32) -> Inventory {
    let mut drives: Vec<_> = ObjectView::all(map)
        .filter(|object| object.has(interface::DRIVE))
        .filter_map(|drive| Some((drive, whole_disk_block(map, drive.path)?)))
        .filter(|(_, block)| block.uint(interface::BLOCK, "Size") > 0)
        .collect();
    drives.sort_unstable_by_key(|(drive, _)| (drive.text(interface::DRIVE, "SortKey"), drive.path));
    let images = ObjectView::all(map)
        .filter(|block| is_user_image(*block, uid))
        .map(|block| disk::from_image(map, block));
    let disks: Vec<Disk> = drives
        .into_iter()
        .map(|(drive, block)| disk::from_drive(map, drive, block))
        .chain(images)
        .collect();
    Inventory {
        disks,
        capabilities: capabilities::capabilities(&map.capabilities),
    }
}

/// The drive's block that is neither a partition nor a cleartext device,
/// preferring one with a partition table.
fn whole_disk_block<'map>(map: &'map ObjectMap, drive_path: &str) -> Option<ObjectView<'map>> {
    ObjectView::all(map)
        .filter(|block| block.object(interface::BLOCK, "Drive") == Some(drive_path))
        .filter(|block| !block.has(interface::PARTITION))
        .filter(|block| {
            block
                .object(interface::BLOCK, "CryptoBackingDevice")
                .is_none()
        })
        .min_by_key(|block| (Reverse(block.has(interface::PARTITION_TABLE)), block.path))
}

fn is_user_image(block: ObjectView<'_>, uid: u32) -> bool {
    block.has(interface::LOOP)
        && !block.has(interface::PARTITION)
        && block.object(interface::BLOCK, "Drive").is_none()
        && !block.flag(interface::BLOCK, "HintIgnore")
        && block.uint(interface::LOOP, "SetupByUID") == u64::from(uid)
        && !block.text(interface::LOOP, "BackingFile").is_empty()
}
