use crate::command::Target;
use crate::model::{BlockId, Content, Disk, Encryption, Inventory, Partition, Unlocked, Volume};

const EXTENDED_PARTITION_TYPES: [&str; 3] = ["0x05", "0x0f", "0x85"];

/// A command target looked up in the inventory.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Resolved<'a> {
    Disk(&'a Disk),
    Block(BlockTarget<'a>),
}

/// A block-level target: a partition, a whole-disk filesystem, or the
/// cleartext device of an unlocked encrypted partition.
#[derive(Debug, Clone, Copy)]
pub(crate) struct BlockTarget<'a> {
    pub disk: &'a Disk,
    pub partition: &'a Partition,
    pub layer: Layer<'a>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Layer<'a> {
    Partition,
    Cleartext(&'a Unlocked),
}

pub(crate) fn resolve<'a>(inventory: &'a Inventory, target: &Target) -> Option<Resolved<'a>> {
    match target {
        Target::Disk(id) => inventory.disk(id).map(Resolved::Disk),
        Target::Block(block) => resolve_block(inventory, block),
    }
}

pub(crate) fn resolve_block<'a>(inventory: &'a Inventory, block: &BlockId) -> Option<Resolved<'a>> {
    inventory.disks.iter().find_map(|disk| {
        disk.partitions()
            .find_map(|partition| block_in_partition(disk, partition, block))
            .map(Resolved::Block)
            .or_else(|| (&disk.block == block).then_some(Resolved::Disk(disk)))
    })
}

fn block_in_partition<'a>(
    disk: &'a Disk,
    partition: &'a Partition,
    block: &BlockId,
) -> Option<BlockTarget<'a>> {
    if &partition.block == block {
        return Some(BlockTarget {
            disk,
            partition,
            layer: Layer::Partition,
        });
    }
    unlocked(&partition.content)
        .filter(|unlocked| &unlocked.cleartext == block)
        .map(|unlocked| BlockTarget {
            disk,
            partition,
            layer: Layer::Cleartext(unlocked),
        })
}

pub(crate) fn unlocked(content: &Content) -> Option<&Unlocked> {
    match content {
        Content::Encrypted(Encryption {
            unlocked: Some(unlocked),
            ..
        }) => Some(unlocked),
        _ => None,
    }
}

impl<'a> BlockTarget<'a> {
    /// Content of the addressed block itself.
    pub fn content(&self) -> &'a Content {
        match self.layer {
            Layer::Partition => &self.partition.content,
            Layer::Cleartext(unlocked) => &unlocked.content,
        }
    }

    /// The filesystem reachable from this block, looking through an unlocked encryption layer.
    pub fn volume(&self) -> Option<&'a Volume> {
        self.content().volume()
    }

    pub fn block(&self) -> &'a BlockId {
        match self.layer {
            Layer::Partition => &self.partition.block,
            Layer::Cleartext(unlocked) => &unlocked.cleartext,
        }
    }

    /// The block carrying the filesystem: the cleartext device for unlocked encryption.
    pub fn filesystem_block(&self) -> &'a BlockId {
        unlocked(&self.partition.content)
            .map_or(&self.partition.block, |unlocked| &unlocked.cleartext)
    }

    pub fn encryption(&self) -> Option<&'a Encryption> {
        match &self.partition.content {
            Content::Encrypted(encryption) => Some(encryption),
            _ => None,
        }
    }

    pub fn is_whole_disk(&self) -> bool {
        self.partition.block == self.disk.block
    }

    pub fn is_cleartext(&self) -> bool {
        matches!(self.layer, Layer::Cleartext(_))
    }

    pub fn is_locked(&self) -> bool {
        !self.is_cleartext()
            && self
                .encryption()
                .is_some_and(|encryption| encryption.unlocked.is_none())
    }

    pub fn is_extended_container(&self) -> bool {
        EXTENDED_PARTITION_TYPES.contains(&self.partition.type_id.as_str())
    }

    pub fn name(&self) -> String {
        volume_name(self.partition, self.volume())
    }
}

/// The name users know a volume by: its label, the GPT partition name, or the device node.
pub(crate) fn volume_name(partition: &Partition, volume: Option<&Volume>) -> String {
    [
        volume.map(|volume| volume.label.as_str()),
        Some(partition.name.as_str()),
    ]
    .into_iter()
    .flatten()
    .find(|name| !name.trim().is_empty())
    .unwrap_or(&partition.device)
    .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guard::test_inventory::{self, ROOT_BLOCK, USB_BLOCK, USB_CLEARTEXT};

    #[test]
    fn resolves_partitions_cleartext_and_disks() {
        let inventory = test_inventory::desktop_with_usb();
        let Some(Resolved::Block(root)) = resolve_block(&inventory, &BlockId::new(ROOT_BLOCK))
        else {
            panic!("root partition not found");
        };
        assert!(!root.is_cleartext());
        assert_eq!(root.name(), "root");

        let Some(Resolved::Block(cleartext)) =
            resolve_block(&inventory, &BlockId::new(USB_CLEARTEXT))
        else {
            panic!("cleartext not found");
        };
        assert!(cleartext.is_cleartext());
        assert_eq!(cleartext.partition.block.object_path(), USB_BLOCK);

        let usb_disk = &inventory.disks[1];
        assert!(
            matches!(resolve_block(&inventory, &usb_disk.block), Some(Resolved::Disk(disk)) if disk.id == usb_disk.id)
        );
        assert!(resolve_block(&inventory, &BlockId::new("/missing")).is_none());
    }

    #[test]
    fn volume_names_fall_back_to_partition_name_then_device() {
        let inventory = test_inventory::desktop_with_usb();
        let partition = inventory.disks[0]
            .partitions()
            .next()
            .expect("efi partition");
        assert_eq!(volume_name(partition, None), "EFI system partition");
        let unnamed = Partition {
            name: String::new(),
            ..partition.clone()
        };
        assert_eq!(volume_name(&unnamed, None), unnamed.device);
    }
}
