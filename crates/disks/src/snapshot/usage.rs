use std::path::{Path, PathBuf};

use crate::model::{Content, Encryption, Inventory, Region, Usage};

/// Fills `Volume.usage` of every mounted volume (including unlocked LUKS
/// contents) from `lookup` of its first mount point.
pub fn fill_usage(inventory: &mut Inventory, lookup: impl Fn(&Path) -> Option<Usage>) {
    inventory
        .disks
        .iter_mut()
        .flat_map(|disk| disk.layout.iter_mut())
        .filter_map(|region| match region {
            Region::Partition(partition) => Some(&mut partition.content),
            Region::Free { .. } => None,
        })
        .for_each(|content| fill_content(content, &lookup));
}

/// The first mount point of every mounted volume, sorted: the paths
/// [`fill_usage`] will look up.
pub fn usage_mount_points(inventory: &Inventory) -> Vec<PathBuf> {
    let mut mount_points: Vec<PathBuf> = inventory
        .disks
        .iter()
        .flat_map(|disk| disk.partitions())
        .filter_map(|partition| partition.content.volume()?.mount_points.first().cloned())
        .collect();
    mount_points.sort_unstable();
    mount_points
}

fn fill_content(content: &mut Content, lookup: &impl Fn(&Path) -> Option<Usage>) {
    match content {
        Content::Filesystem(volume) => {
            volume.usage = volume
                .mount_points
                .first()
                .and_then(|mount_point| lookup(mount_point));
        }
        Content::Encrypted(Encryption {
            unlocked: Some(unlocked),
            ..
        }) => fill_content(&mut unlocked.content, lookup),
        _ => {}
    }
}
