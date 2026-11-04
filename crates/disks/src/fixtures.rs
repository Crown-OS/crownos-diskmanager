//! Deterministic inventories from recorded and hand-derived UDisks2 fixtures,
//! for UI and policy tests. Every fixture includes this dev host's desktop
//! (two NVMe drives; root-owned Waydroid loops and zram, which are hidden).

use std::path::Path;

use crate::model::{ByteSize, Inventory, Usage};
use crate::object_map::ObjectMap;
use crate::snapshot;

/// The user the fixtures were recorded for.
pub const FIXTURE_UID: u32 = 1000;

pub(crate) mod json {
    pub const DESKTOP: &str = include_str!("../tests/fixtures/desktop.json");
    pub const USB_STICK: &str = include_str!("../tests/fixtures/usb_stick.json");
    pub const ISO_IMAGE: &str = include_str!("../tests/fixtures/iso_image.json");
    pub const ATA: &str = include_str!("../tests/fixtures/ata.json");
    pub const ENCRYPTED: &str = include_str!("../tests/fixtures/encrypted.json");
    pub const BLANK_DISK: &str = include_str!("../tests/fixtures/blank_disk.json");
}

const GB: u64 = 1_000_000_000;

/// `(mount point, total, used)` reported for mounted fixture volumes.
const FIXTURE_USAGE: [(&str, u64, u64); 7] = [
    ("/", 503 * GB, 312 * GB),
    ("/boot", 1_070_000_000, 180_000_000),
    ("/e", 492 * GB, 458 * GB),
    ("/run/media/marvelxcodes/SANDISK", 61 * GB, 12 * GB),
    (
        "/run/media/marvelxcodes/CROWNOS_202611",
        2_147_483_648,
        2_147_483_648,
    ),
    ("/run/media/marvelxcodes/Photos", 634 * GB, 210 * GB),
    ("/mnt/projects", 214 * GB, 97 * GB),
];

/// This dev host: a Samsung and a Crucial NVMe drive, ext4 and vfat.
pub fn desktop() -> Inventory {
    inventory(&[])
}

/// Desktop plus a SanDisk USB stick (exFAT, MBR) and a user-opened ISO image.
pub fn removable() -> Inventory {
    inventory(&[json::USB_STICK, json::ISO_IMAGE])
}

/// Desktop plus a USB SSD with a locked and an unlocked (mounted ext4) LUKS2 partition.
pub fn encrypted() -> Inventory {
    inventory(&[json::ENCRYPTED])
}

/// Desktop plus a healthy SATA SSD (free-space gaps, btrfs, active swap) and
/// a failing SATA HDD with bad sectors (MBR, NTFS, extended + LVM member).
pub fn ata_failing() -> Inventory {
    inventory(&[json::ATA])
}

/// Desktop plus an internal HDD with no partition table and no content.
pub fn blank_disk() -> Inventory {
    inventory(&[json::BLANK_DISK])
}

fn inventory(extra_fixtures: &[&str]) -> Inventory {
    let mut inventory = snapshot::build(&merged_object_map(extra_fixtures), FIXTURE_UID);
    snapshot::fill_usage(&mut inventory, fixture_usage);
    inventory
}

/// Desktop with `extra_fixtures` layered on top; malformed JSON is caught by
/// the snapshot tests, which parse every fixture.
pub(crate) fn merged_object_map(extra_fixtures: &[&str]) -> ObjectMap {
    let parse = |json| serde_json::from_str::<ObjectMap>(json).unwrap_or_default();
    extra_fixtures
        .iter()
        .map(|json| parse(json))
        .fold(parse(json::DESKTOP), |mut map, extra| {
            map.objects.extend(extra.objects);
            map.nvme_attributes.extend(extra.nvme_attributes);
            map
        })
}

fn fixture_usage(mount_point: &Path) -> Option<Usage> {
    FIXTURE_USAGE
        .iter()
        .find(|(path, ..)| Path::new(path) == mount_point)
        .map(|&(_, total, used)| Usage {
            total: ByteSize(total),
            used: ByteSize(used),
        })
}
