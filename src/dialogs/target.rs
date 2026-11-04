use std::borrow::Cow;

use crownos_disks::command::Target;
use crownos_disks::model::{BlockId, ByteSize, Content, Disk, Inventory, Partition};

use crate::state::DialogRequest;
use crate::util::region_text::partition_title;

#[derive(Debug, Clone, Copy)]
pub enum Resolved<'a> {
    Disk(&'a Disk),
    Partition(&'a Disk, &'a Partition),
}

impl<'a> Resolved<'a> {
    pub fn find(inventory: &'a Inventory, target: &Target) -> Option<Self> {
        match target {
            Target::Disk(id) => inventory.disk(id).map(Self::Disk),
            Target::Block(block) => Self::find_block(inventory, block),
        }
    }

    fn find_block(inventory: &'a Inventory, block: &BlockId) -> Option<Self> {
        inventory
            .partition(block)
            .map(|(disk, partition)| Self::Partition(disk, partition))
            .or_else(|| {
                inventory
                    .disks
                    .iter()
                    .find(|disk| &disk.block == block)
                    .map(Self::Disk)
            })
    }

    pub fn disk(self) -> &'a Disk {
        match self {
            Self::Disk(disk) | Self::Partition(disk, _) => disk,
        }
    }

    pub fn partition(self) -> Option<&'a Partition> {
        match self {
            Self::Disk(_) => None,
            Self::Partition(_, partition) => Some(partition),
        }
    }

    pub fn name(self) -> Cow<'a, str> {
        match self {
            Self::Disk(disk) => Cow::Borrowed(&disk.name),
            Self::Partition(disk, partition) if partition.is_whole_disk() => {
                match partition
                    .content
                    .volume()
                    .map(|volume| volume.label.as_str())
                    .filter(|label| !label.is_empty())
                {
                    Some(label) => Cow::Borrowed(label),
                    None => Cow::Borrowed(&disk.name),
                }
            }
            Self::Partition(_, partition) => partition_title(partition),
        }
    }

    pub fn size(self) -> ByteSize {
        match self {
            Self::Disk(disk) => disk.size,
            Self::Partition(_, partition) => partition.size,
        }
    }

    pub fn content(self) -> Option<&'a Content> {
        self.partition().map(|partition| &partition.content)
    }
}

pub fn request_target(request: &DialogRequest) -> Option<Target> {
    match request {
        DialogRequest::EraseDisk(disk) | DialogRequest::NewPartition { disk, .. } => {
            Some(Target::Disk(disk.clone()))
        }
        DialogRequest::Format(block)
        | DialogRequest::Resize(block)
        | DialogRequest::Rename(block)
        | DialogRequest::Unlock(block)
        | DialogRequest::Delete(block)
        | DialogRequest::Repair(block) => Some(Target::Block(block.clone())),
        DialogRequest::CreateImage(target) | DialogRequest::RestoreImage(target) => {
            Some(target.clone())
        }
        DialogRequest::OpenImage | DialogRequest::QuitWhileBusy => None,
    }
}

pub fn target_name(inventory: &Inventory, target: &Target) -> Option<String> {
    Resolved::find(inventory, target).map(|resolved| resolved.name().into_owned())
}
