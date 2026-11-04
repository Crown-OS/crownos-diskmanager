use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{BlockId, ByteSize, DiskId, DiskKind, Health, Partition, Region};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Disk {
    pub id: DiskId,
    pub block: BlockId,
    pub device: String,
    pub kind: DiskKind,
    pub name: String,
    pub vendor: String,
    pub model: String,
    pub serial: String,
    pub revision: String,
    pub size: ByteSize,
    pub connection: String,
    pub rotation_rate: Option<u32>,
    pub removal: Removal,
    pub table: Option<TableKind>,
    pub table_uuid: String,
    pub layout: Vec<Region>,
    pub health: Health,
    pub backing_file: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Removal {
    Fixed,
    Removable {
        ejectable: bool,
        can_power_off: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TableKind {
    Gpt,
    Mbr,
}

impl Disk {
    pub fn partitions(&self) -> impl Iterator<Item = &Partition> {
        self.layout.iter().filter_map(|region| match region {
            Region::Partition(partition) => Some(partition),
            Region::Free { .. } => None,
        })
    }

    pub fn partition(&self, block: &BlockId) -> Option<&Partition> {
        self.partitions()
            .find(|partition| &partition.block == block)
    }
}
