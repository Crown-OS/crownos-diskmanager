use crownos_disks::model::{BlockId, ByteSize, Disk, DiskId, DiskKind, Inventory, Removal};

use crate::util::region_text::{content_summary, is_locked, partition_title};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Internal,
    External,
    Images,
}

impl Category {
    pub const ALL: [Self; 3] = [Self::Internal, Self::External, Self::Images];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Internal => "Internal",
            Self::External => "External",
            Self::Images => "Disk Images",
        }
    }

    pub const fn of(kind: DiskKind) -> Self {
        match kind {
            DiskKind::Image => Self::Images,
            kind if kind.is_external() => Self::External,
            _ => Self::Internal,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Detach {
    Fixed,
    Eject,
    CloseImage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumeState {
    Mounted,
    Unmounted,
    Locked,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SidebarDisk {
    pub id: DiskId,
    pub name: String,
    pub size: ByteSize,
    pub kind: DiskKind,
    pub detach: Detach,
    pub volumes: Vec<SidebarVolume>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SidebarVolume {
    pub block: BlockId,
    pub title: String,
    pub content: String,
    pub state: VolumeState,
}

pub fn sidebar_disks(inventory: &Inventory, category: Category) -> Vec<SidebarDisk> {
    inventory
        .disks
        .iter()
        .filter(|disk| Category::of(disk.kind) == category)
        .map(sidebar_disk)
        .collect()
}

fn sidebar_disk(disk: &Disk) -> SidebarDisk {
    let detach = match (disk.kind, disk.removal) {
        (DiskKind::Image, _) => Detach::CloseImage,
        (_, Removal::Removable { .. }) => Detach::Eject,
        (_, Removal::Fixed) => Detach::Fixed,
    };
    let volumes = disk
        .partitions()
        .map(|partition| SidebarVolume {
            block: partition.block.clone(),
            title: partition_title(partition).into_owned(),
            content: content_summary(&partition.content).into_owned(),
            state: match partition.content.volume() {
                _ if is_locked(&partition.content) => VolumeState::Locked,
                Some(volume) if volume.is_mounted() => VolumeState::Mounted,
                _ => VolumeState::Unmounted,
            },
        })
        .collect();
    SidebarDisk {
        id: disk.id.clone(),
        name: disk.name.clone(),
        size: disk.size,
        kind: disk.kind,
        detach,
        volumes,
    }
}
