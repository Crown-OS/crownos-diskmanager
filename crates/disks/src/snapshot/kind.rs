use super::object_view::ObjectView;
use crate::model::DiskKind;
use crate::object_map::interface;

const USB_BUS: &str = "usb";
const SDIO_BUS: &str = "sdio";
const MMC_DEVICE_PREFIX: &str = "/dev/mmcblk";

/// Classifies a disk, first match wins:
/// Loop → Image, Optical, usb → Usb, sdio/mmcblk → SdCard, NVMe controller → Nvme,
/// RotationRate 0 → Ssd, otherwise Hdd.
pub(super) fn classify(drive: Option<ObjectView<'_>>, block: ObjectView<'_>) -> DiskKind {
    if block.has(interface::LOOP) {
        return DiskKind::Image;
    }
    let Some(drive) = drive else {
        return DiskKind::Image;
    };
    let bus = drive.text(interface::DRIVE, "ConnectionBus");
    let is_mmc = block
        .text(interface::BLOCK, "Device")
        .starts_with(MMC_DEVICE_PREFIX);
    if drive.flag(interface::DRIVE, "Optical") {
        DiskKind::Optical
    } else if bus == USB_BUS {
        DiskKind::Usb
    } else if bus == SDIO_BUS || is_mmc {
        DiskKind::SdCard
    } else if drive.has(interface::NVME_CONTROLLER) {
        DiskKind::Nvme
    } else if drive.int(interface::DRIVE, "RotationRate") == Some(0) {
        DiskKind::Ssd
    } else {
        DiskKind::Hdd
    }
}
