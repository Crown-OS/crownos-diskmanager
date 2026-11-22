use std::path::{Path, PathBuf};

use crownos_disks::fixtures;
use crownos_disks::model::{
    BlockId, ByteSize, Content, Disk, DiskId, Health, HealthReport, HealthStatus, Inventory,
    Partition, Region, Usage,
};

const RECORDED_MEDIA: &str = "/run/media/marvelxcodes";
const SAMPLE_MEDIA: &str = "/run/media/alex";
const RECORDED_HOME: &str = "/home/marvelxcodes";
const SAMPLE_HOME: &str = "/home/alex";
const RECORDED_WORKSPACE: &str = "/e";
const SAMPLE_WORKSPACE: &str = "/mnt/workspace";
const DRIVE_PATH: &str = "/org/freedesktop/UDisks2/drives";

pub const SYSTEM_DISK: &str = "Samsung SSD 980 PRO";
pub const WORKSPACE_DISK: &str = "WD Black SN770";
pub const USB_STICK: &str = "SanDisk Ultra";
pub const DATA_VOLUME: &str = "Data";
const DATA_USED_BYTES: u64 = 38_400_000_000;

struct SampleDrive {
    recorded_model: &'static str,
    name: &'static str,
    vendor: &'static str,
    model: &'static str,
    revision: &'static str,
    power_on_hours: u64,
    unsafe_shutdowns: u64,
}

const SAMPLE_DRIVES: [SampleDrive; 2] = [
    SampleDrive {
        recorded_model: "SAMSUNG MZVL2512HDJD-00BL2",
        name: SYSTEM_DISK,
        vendor: "Samsung",
        model: "Samsung SSD 980 PRO 500GB",
        revision: "5B2QGXA7",
        power_on_hours: 1843,
        unsafe_shutdowns: 12,
    },
    SampleDrive {
        recorded_model: "CT500P3PSSD8",
        name: WORKSPACE_DISK,
        vendor: "Western Digital",
        model: "WDS500G3X0E-00B3N0",
        revision: "731100WD",
        power_on_hours: 962,
        unsafe_shutdowns: 4,
    },
];

/// One sample serial per device node, matching the brand the fixtures put there.
const SAMPLE_SERIALS: [(&str, &str); 7] = [
    ("/dev/nvme0n1", "23164B801476"),
    ("/dev/nvme1n1", "S5GXNF0R614392K"),
    ("/dev/sda", "4C531001580721109284"),
    ("/dev/sdb", "S6PXNM0T778201B"),
    ("/dev/sdc", "ZN1C8Q4T"),
    ("/dev/sdd", "S5SXNS0R412808H"),
    ("/dev/sde", "WD-WCC4M7K2Q1X9"),
];

/// The developer's desktop plus a USB stick and an opened ISO, with every recorded identity
/// (drive models, serials, mount points, home folder) replaced by sample values.
pub fn removable() -> Inventory {
    sample(fixtures::removable())
}

/// A USB SSD with one locked and one unlocked LUKS partition.
pub fn encrypted() -> Inventory {
    sample(fixtures::encrypted())
}

/// A SATA SSD with free space and a failing hard disk.
pub fn failing() -> Inventory {
    sample(fixtures::ata_failing())
}

/// An internal hard disk with no partition table.
pub fn blank() -> Inventory {
    sample(fixtures::blank_disk())
}

/// The failing set, where the SATA SSD's unmounted "Data" volume reports what it holds, so it
/// can shrink as well as grow.
pub fn failing_with_known_usage() -> Inventory {
    let mut inventory = failing();
    let data = inventory
        .disks
        .iter_mut()
        .flat_map(|disk| disk.layout.iter_mut())
        .find_map(|region| match region {
            Region::Partition(partition) if labelled(DATA_VOLUME)(partition) => Some(partition),
            _ => None,
        });
    if let Some(partition) = data {
        let total = partition.size;
        if let Content::Filesystem(volume) = &mut partition.content {
            volume.usage = Some(Usage {
                total,
                used: ByteSize(DATA_USED_BYTES),
            });
        }
    }
    inventory
}

/// Nothing but the system disk.
pub fn system_only() -> Inventory {
    let mut inventory = removable();
    inventory.disks.retain(|disk| disk.name == SYSTEM_DISK);
    inventory
}

pub fn name_where(inventory: &Inventory, matches: impl Fn(&Disk) -> bool) -> Option<String> {
    inventory
        .disks
        .iter()
        .find(|disk| matches(disk))
        .map(|disk| disk.name.clone())
}

pub fn health_status(disk: &Disk) -> Option<HealthStatus> {
    match &disk.health {
        Health::Reported(report) => Some(report.status),
        Health::Unavailable => None,
    }
}

pub fn disk_named(inventory: &Inventory, name: &str) -> Option<DiskId> {
    inventory
        .disks
        .iter()
        .find(|disk| disk.name == name)
        .map(|disk| disk.id.clone())
}

/// The partition of disk `disk` that `matches`.
pub fn partition_where(
    inventory: &Inventory,
    disk: &str,
    matches: impl Fn(&Partition) -> bool,
) -> Option<(DiskId, BlockId)> {
    let disk = inventory
        .disks
        .iter()
        .find(|candidate| candidate.name == disk)?;
    disk.partitions()
        .find(|partition| matches(partition))
        .map(|partition| (disk.id.clone(), partition.block.clone()))
}

/// The partition on any disk that `matches`.
pub fn partition_where_any(
    inventory: &Inventory,
    matches: impl Fn(&Partition) -> bool,
) -> Option<(DiskId, BlockId)> {
    inventory.disks.iter().find_map(|disk| {
        disk.partitions()
            .find(|partition| matches(partition))
            .map(|partition| (disk.id.clone(), partition.block.clone()))
    })
}

/// The partition whose volume label, or partition name, is `label`.
pub fn labelled(label: &str) -> impl Fn(&Partition) -> bool + '_ {
    move |partition| {
        partition.name == label
            || partition
                .content
                .volume()
                .is_some_and(|volume| volume.label == label)
    }
}

pub fn mounted_at(mount_point: &str) -> impl Fn(&Partition) -> bool + '_ {
    move |partition| {
        partition.content.volume().is_some_and(|volume| {
            volume
                .mount_points
                .iter()
                .any(|path| path == Path::new(mount_point))
        })
    }
}

/// The first free region of disk `disk`.
pub fn free_region(
    inventory: &Inventory,
    disk: &str,
) -> Option<(
    DiskId,
    crownos_disks::model::ByteSize,
    crownos_disks::model::ByteSize,
)> {
    let disk = inventory
        .disks
        .iter()
        .find(|candidate| candidate.name == disk)?;
    disk.layout.iter().find_map(|region| match region {
        Region::Free { offset, size } => Some((disk.id.clone(), *offset, *size)),
        Region::Partition(_) => None,
    })
}

/// The sample desktop as recorded, where most formatting tools are not installed.
pub fn removable_missing_tools() -> Inventory {
    let mut inventory = removable();
    inventory.capabilities = fixtures::removable().capabilities;
    inventory
}

/// Every disk renamed to sample values, on a system with every formatting tool installed.
fn sample(mut inventory: Inventory) -> Inventory {
    inventory.disks.iter_mut().for_each(sample_disk);
    inventory
        .capabilities
        .formats
        .iter_mut()
        .for_each(|support| support.missing_package = None);
    inventory
}

fn sample_disk(disk: &mut Disk) {
    if let Some(drive) = SAMPLE_DRIVES
        .iter()
        .find(|drive| drive.recorded_model == disk.model)
    {
        disk.name = drive.name.to_owned();
        disk.vendor = drive.vendor.to_owned();
        disk.model = drive.model.to_owned();
        disk.revision = drive.revision.to_owned();
        disk.id = DiskId::new(format!("{DRIVE_PATH}/{}", drive.name.replace(' ', "_")));
        disk.health = Health::Reported(HealthReport {
            status: HealthStatus::Good,
            temperature_celsius: Some(38.0),
            power_on_hours: Some(drive.power_on_hours),
            life_used_percent: Some(2),
            spare_percent: Some(100),
            media_errors: Some(0),
            unsafe_shutdowns: Some(drive.unsafe_shutdowns),
            bad_sectors: None,
            notes: Vec::new(),
        });
    }
    disk.serial = sample_serial(&disk.serial, &disk.device);
    disk.table_uuid = format!(
        "5a3c9e10-7b4d-4f2a-9c61-{:012x}",
        disk.size.bytes() % 0xffff_ffff_ffff
    );
    disk.backing_file = disk.backing_file.take().map(|path| rebase(&path));
    for region in &mut disk.layout {
        if let Region::Partition(partition) = region {
            partition.uuid = format!(
                "8f1d2c3b-4a5e-4b6c-8d7e-{:012x}",
                partition.offset.bytes() % 0xffff_ffff_ffff
            );
            sample_content(&mut partition.content);
        }
    }
}

/// The sample serial of `device`; disk images, which report none, keep none.
fn sample_serial(recorded: &str, device: &str) -> String {
    match recorded.is_empty() {
        true => String::new(),
        false => SAMPLE_SERIALS
            .iter()
            .find(|(node, _)| *node == device)
            .map_or("CRN00000001", |(_, serial)| serial)
            .to_owned(),
    }
}

fn sample_content(content: &mut Content) {
    match content {
        Content::Filesystem(volume) => {
            volume.uuid = "3e1f0c2a-9b7d-4e5f-a1c2-6d8e0f1a2b3c".to_owned();
            volume.mount_points = volume
                .mount_points
                .iter()
                .map(|path| rebase(path))
                .collect();
        }
        Content::Encrypted(encryption) => {
            if let Some(unlocked) = encryption.unlocked.as_mut() {
                sample_content(&mut unlocked.content);
            }
        }
        _ => {}
    }
}

fn rebase(path: &Path) -> PathBuf {
    [(RECORDED_MEDIA, SAMPLE_MEDIA), (RECORDED_HOME, SAMPLE_HOME)]
        .into_iter()
        .find_map(|(recorded, sample)| {
            path.strip_prefix(recorded)
                .ok()
                .map(|rest| Path::new(sample).join(rest))
        })
        .unwrap_or_else(|| match path == Path::new(RECORDED_WORKSPACE) {
            true => PathBuf::from(SAMPLE_WORKSPACE),
            false => path.to_owned(),
        })
}
