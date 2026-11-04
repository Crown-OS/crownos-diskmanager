use serde::{Deserialize, Serialize};

use super::{BlockId, ByteSize, Volume};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(clippy::large_enum_variant)]
pub enum Region {
    Partition(Partition),
    Free { offset: ByteSize, size: ByteSize },
}

impl Region {
    pub const fn offset(&self) -> ByteSize {
        match self {
            Self::Partition(partition) => partition.offset,
            Self::Free { offset, .. } => *offset,
        }
    }

    pub const fn size(&self) -> ByteSize {
        match self {
            Self::Partition(partition) => partition.size,
            Self::Free { size, .. } => *size,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Partition {
    pub block: BlockId,
    pub device: String,
    pub number: u32,
    pub offset: ByteSize,
    pub size: ByteSize,
    pub name: String,
    pub type_id: String,
    pub uuid: String,
    pub content: Content,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Content {
    Filesystem(Volume),
    Encrypted(Encryption),
    Swap { active: bool },
    Member { kind: String },
    Unknown { id_type: String },
    Empty,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Encryption {
    pub version: LuksVersion,
    pub header_size: ByteSize,
    pub unlocked: Option<Box<Unlocked>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LuksVersion {
    Luks1,
    Luks2,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Unlocked {
    pub cleartext: BlockId,
    pub content: Content,
}

impl Content {
    pub fn volume(&self) -> Option<&Volume> {
        match self {
            Self::Filesystem(volume) => Some(volume),
            Self::Encrypted(Encryption {
                unlocked: Some(unlocked),
                ..
            }) => unlocked.content.volume(),
            _ => None,
        }
    }
}
