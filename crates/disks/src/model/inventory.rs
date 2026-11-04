use serde::{Deserialize, Serialize};

use super::{BlockId, Disk, DiskId, FilesystemKind, Partition};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Inventory {
    pub disks: Vec<Disk>,
    pub capabilities: Capabilities,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Capabilities {
    pub formats: Vec<FormatSupport>,
    pub resize: Vec<(FilesystemKind, ResizeModes)>,
    pub encryption: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FormatSupport {
    pub filesystem: FilesystemKind,
    pub missing_package: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResizeModes {
    pub offline_shrink: bool,
    pub offline_grow: bool,
    pub online_shrink: bool,
    pub online_grow: bool,
}

impl Inventory {
    pub fn disk(&self, id: &DiskId) -> Option<&Disk> {
        self.disks.iter().find(|disk| &disk.id == id)
    }

    pub fn partition(&self, block: &BlockId) -> Option<(&Disk, &Partition)> {
        self.disks
            .iter()
            .find_map(|disk| disk.partition(block).map(|partition| (disk, partition)))
    }

    pub fn resize_modes(&self, filesystem: &FilesystemKind) -> ResizeModes {
        self.capabilities
            .resize
            .iter()
            .find_map(|(kind, modes)| (kind == filesystem).then_some(*modes))
            .unwrap_or_default()
    }
}
