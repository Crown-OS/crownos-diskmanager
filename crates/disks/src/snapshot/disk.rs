use std::path::{Path, PathBuf};

use super::layout::{self, Layout};
use super::object_view::ObjectView;
use super::{health, kind};
use crate::model::{BlockId, ByteSize, Disk, DiskId, DiskKind, Health, Removal};
use crate::object_map::{ObjectMap, interface};

const NVME_CONNECTION: &str = "nvme";
const LOOP_CONNECTION: &str = "loop";

/// A physical drive and its whole-disk block device.
pub(super) fn from_drive(map: &ObjectMap, drive: ObjectView<'_>, block: ObjectView<'_>) -> Disk {
    let kind = kind::classify(Some(drive), block);
    let vendor = drive.text(interface::DRIVE, "Vendor").trim();
    let model = drive.text(interface::DRIVE, "Model").trim();
    let size = ByteSize(block.uint(interface::BLOCK, "Size"));
    let bus = drive.text(interface::DRIVE, "ConnectionBus");
    let connection = match (bus, kind) {
        ("", DiskKind::Nvme) => NVME_CONNECTION,
        _ => bus,
    };
    let Layout { table, regions } = layout::layout(map, block);
    Disk {
        id: DiskId::new(drive.path),
        block: BlockId::new(block.path),
        device: layout::device(block),
        kind,
        name: friendly_name(vendor, model, size, kind),
        vendor: vendor.to_owned(),
        model: model.to_owned(),
        serial: drive.text(interface::DRIVE, "Serial").to_owned(),
        revision: drive.text(interface::DRIVE, "Revision").to_owned(),
        size,
        connection: connection.to_owned(),
        rotation_rate: drive
            .int(interface::DRIVE, "RotationRate")
            .and_then(|rate| u32::try_from(rate).ok()),
        removal: removal(drive),
        table,
        table_uuid: block.text(interface::BLOCK, "IdUUID").to_owned(),
        layout: regions,
        health: health::drive_health(map, drive),
        backing_file: None,
    }
}

/// A loop device backed by an image file.
pub(super) fn from_image(map: &ObjectMap, block: ObjectView<'_>) -> Disk {
    let backing_file = PathBuf::from(block.text(interface::LOOP, "BackingFile"));
    let size = ByteSize(block.uint(interface::BLOCK, "Size"));
    let Layout { table, regions } = layout::layout(map, block);
    Disk {
        id: DiskId::new(block.path),
        block: BlockId::new(block.path),
        device: layout::device(block),
        kind: DiskKind::Image,
        name: image_name(&backing_file, block),
        vendor: String::new(),
        model: String::new(),
        serial: String::new(),
        revision: String::new(),
        size,
        connection: LOOP_CONNECTION.to_owned(),
        rotation_rate: None,
        removal: Removal::Removable {
            ejectable: false,
            can_power_off: false,
        },
        table,
        table_uuid: block.text(interface::BLOCK, "IdUUID").to_owned(),
        layout: regions,
        health: Health::Unavailable,
        backing_file: Some(backing_file),
    }
}

fn removal(drive: ObjectView<'_>) -> Removal {
    let ejectable = drive.flag(interface::DRIVE, "Ejectable");
    let can_power_off = drive.flag(interface::DRIVE, "CanPowerOff");
    let removable =
        drive.flag(interface::DRIVE, "Removable") || drive.flag(interface::DRIVE, "MediaRemovable");
    match removable || ejectable || can_power_off {
        true => Removal::Removable {
            ejectable,
            can_power_off,
        },
        false => Removal::Fixed,
    }
}

/// "Vendor Model", without repeating a vendor the model already starts with,
/// falling back to "64 GB USB Drive" when the drive reports neither.
fn friendly_name(vendor: &str, model: &str, size: ByteSize, kind: DiskKind) -> String {
    let model_has_vendor = model
        .get(..vendor.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(vendor));
    match (vendor, model) {
        ("", "") => format!("{size} {}", kind.label()),
        ("", model) => model.to_owned(),
        (vendor, "") => vendor.to_owned(),
        (_, model) if model_has_vendor => model.to_owned(),
        (vendor, model) => format!("{vendor} {model}"),
    }
}

fn image_name(backing_file: &Path, block: ObjectView<'_>) -> String {
    let label = block.text(interface::BLOCK, "IdLabel");
    backing_file
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .or_else(|| (!label.is_empty()).then(|| label.to_owned()))
        .unwrap_or_else(|| layout::device(block))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn friendly_names_join_vendor_and_model_once() {
        let size = ByteSize(64_000_000_000);
        assert_eq!(
            friendly_name("SanDisk", "Ultra", size, DiskKind::Usb),
            "SanDisk Ultra"
        );
        assert_eq!(
            friendly_name("", "CT500P3PSSD8", size, DiskKind::Nvme),
            "CT500P3PSSD8"
        );
        assert_eq!(
            friendly_name("WDC", "WDC WD10EZEX", size, DiskKind::Hdd),
            "WDC WD10EZEX"
        );
        assert_eq!(
            friendly_name("", "", size, DiskKind::Usb),
            "64.0 GB USB Drive"
        );
    }
}
