use std::sync::Arc;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FilesystemKind {
    Ext4,
    Ext3,
    Ext2,
    Btrfs,
    Xfs,
    Fat32,
    Exfat,
    Ntfs,
    F2fs,
    Swap,
    Other(Arc<str>),
}

impl FilesystemKind {
    pub const FORMATTABLE: [Self; 8] = [
        Self::Ext4,
        Self::Exfat,
        Self::Fat32,
        Self::Ntfs,
        Self::Btrfs,
        Self::Xfs,
        Self::F2fs,
        Self::Swap,
    ];

    pub fn from_udisks(id_type: &str) -> Self {
        match id_type {
            "ext4" => Self::Ext4,
            "ext3" => Self::Ext3,
            "ext2" => Self::Ext2,
            "btrfs" => Self::Btrfs,
            "xfs" => Self::Xfs,
            "vfat" => Self::Fat32,
            "exfat" => Self::Exfat,
            "ntfs" => Self::Ntfs,
            "f2fs" => Self::F2fs,
            "swap" => Self::Swap,
            other => Self::Other(other.into()),
        }
    }

    pub fn udisks_type(&self) -> &str {
        match self {
            Self::Ext4 => "ext4",
            Self::Ext3 => "ext3",
            Self::Ext2 => "ext2",
            Self::Btrfs => "btrfs",
            Self::Xfs => "xfs",
            Self::Fat32 => "vfat",
            Self::Exfat => "exfat",
            Self::Ntfs => "ntfs",
            Self::F2fs => "f2fs",
            Self::Swap => "swap",
            Self::Other(name) => name,
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            Self::Ext4 => "Linux (ext4)",
            Self::Exfat => "All devices (exFAT)",
            Self::Fat32 => "Older devices (FAT32)",
            Self::Ntfs => "Windows (NTFS)",
            Self::Btrfs => "Btrfs (snapshots)",
            Self::Xfs => "XFS (large files)",
            Self::F2fs => "F2FS (flash storage)",
            Self::Swap => "Swap",
            other => other.short_name(),
        }
    }

    pub fn short_name(&self) -> &str {
        match self {
            Self::Ext4 => "ext4",
            Self::Ext3 => "ext3",
            Self::Ext2 => "ext2",
            Self::Btrfs => "Btrfs",
            Self::Xfs => "XFS",
            Self::Fat32 => "FAT32",
            Self::Exfat => "exFAT",
            Self::Ntfs => "NTFS",
            Self::F2fs => "F2FS",
            Self::Swap => "Swap",
            Self::Other(name) => name,
        }
    }

    pub const fn is_linux_native(&self) -> bool {
        matches!(
            self,
            Self::Ext4 | Self::Ext3 | Self::Ext2 | Self::Btrfs | Self::Xfs | Self::F2fs
        )
    }
}
