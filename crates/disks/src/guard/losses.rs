use super::Loss;
use super::target::{Resolved, resolve, volume_name};
use crate::command::Target;
use crate::model::{Content, FilesystemKind, Inventory, Partition};

pub(super) fn losses(inventory: &Inventory, target: &Target) -> Vec<Loss> {
    match resolve(inventory, target) {
        None => Vec::new(),
        Some(Resolved::Disk(disk)) => disk
            .partitions()
            .filter_map(|partition| loss(partition, &partition.content))
            .collect(),
        Some(Resolved::Block(block)) => {
            loss(block.partition, block.content()).into_iter().collect()
        }
    }
}

fn loss(partition: &Partition, content: &Content) -> Option<Loss> {
    let opaque = |filesystem: Option<FilesystemKind>| Loss {
        name: volume_name(partition, None),
        filesystem,
        used: None,
    };
    match content {
        Content::Empty => None,
        Content::Filesystem(volume) => Some(Loss {
            name: volume_name(partition, Some(volume)),
            filesystem: Some(volume.filesystem.clone()),
            used: volume.usage.map(|usage| usage.used),
        }),
        Content::Encrypted(encryption) => encryption
            .unlocked
            .as_ref()
            .and_then(|unlocked| loss(partition, &unlocked.content))
            .or_else(|| Some(opaque(None))),
        Content::Swap { .. } => Some(opaque(Some(FilesystemKind::Swap))),
        Content::Member { .. } | Content::Unknown { .. } => Some(opaque(None)),
    }
}
