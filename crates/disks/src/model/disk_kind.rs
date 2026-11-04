use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DiskKind {
    Nvme,
    Ssd,
    Hdd,
    Usb,
    SdCard,
    Optical,
    Image,
}

impl DiskKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Nvme => "NVMe SSD",
            Self::Ssd => "SSD",
            Self::Hdd => "Hard Disk",
            Self::Usb => "USB Drive",
            Self::SdCard => "SD Card",
            Self::Optical => "Optical Drive",
            Self::Image => "Disk Image",
        }
    }

    pub const fn is_external(self) -> bool {
        matches!(self, Self::Usb | Self::SdCard | Self::Optical)
    }
}
