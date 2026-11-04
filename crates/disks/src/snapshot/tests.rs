use std::path::{Path, PathBuf};

use super::build;
use crate::fixtures::{self, FIXTURE_UID, json};
use crate::model::{
    ByteSize, Content, Disk, DiskKind, Encryption, FilesystemKind, Health, HealthNote,
    HealthStatus, Inventory, LuksVersion, Region, Removal, TableKind,
};
use crate::object_map::ObjectMap;

const GIB: u64 = ByteSize::GIB.bytes();
const MIB: u64 = ByteSize::MIB.bytes();

fn disk<'inventory>(inventory: &'inventory Inventory, device: &str) -> &'inventory Disk {
    inventory
        .disks
        .iter()
        .find(|disk| disk.device == device)
        .unwrap_or_else(|| panic!("{device} missing"))
}

fn summary(disk: &Disk) -> Vec<String> {
    disk.layout
        .iter()
        .map(|region| match region {
            Region::Free { offset, size } => format!("free {}..{}", offset.0, offset.0 + size.0),
            Region::Partition(partition) => format!(
                "{} {:?}",
                partition.device,
                partition.content.volume().map(|v| &v.filesystem)
            ),
        })
        .collect()
}

#[test]
fn every_fixture_parses() {
    for fixture in [
        json::DESKTOP,
        json::USB_STICK,
        json::ISO_IMAGE,
        json::ATA,
        json::ENCRYPTED,
        json::BLANK_DISK,
    ] {
        serde_json::from_str::<ObjectMap>(fixture).unwrap();
    }
}

#[test]
fn desktop_lists_both_nvme_drives_and_hides_system_loops_and_zram() {
    let inventory = fixtures::desktop();
    let devices: Vec<_> = inventory
        .disks
        .iter()
        .map(|disk| disk.device.as_str())
        .collect();
    assert_eq!(devices, ["/dev/nvme0n1", "/dev/nvme1n1"]);

    let samsung = disk(&inventory, "/dev/nvme1n1");
    assert_eq!(samsung.kind, DiskKind::Nvme);
    assert_eq!(samsung.name, "SAMSUNG MZVL2512HDJD-00BL2");
    assert_eq!(samsung.connection, "nvme");
    assert_eq!(samsung.removal, Removal::Fixed);
    assert_eq!(samsung.table, Some(TableKind::Gpt));
    assert_eq!(samsung.size, ByteSize(512_110_190_592));
    assert_eq!(
        summary(samsung),
        ["/dev/nvme1n1p5 Some(Fat32)", "/dev/nvme1n1p6 Some(Ext4)"]
    );

    let root = samsung
        .partitions()
        .nth(1)
        .and_then(|partition| partition.content.volume())
        .unwrap();
    assert_eq!(root.mount_points, [PathBuf::from("/")]);
    assert_eq!(
        root.usage.map(|usage| usage.used),
        Some(ByteSize(312_000_000_000))
    );
}

#[test]
fn desktop_health_comes_from_nvme_smart() {
    let inventory = fixtures::desktop();
    let Health::Reported(report) = &disk(&inventory, "/dev/nvme0n1").health else {
        panic!("expected NVMe health");
    };
    assert_eq!(report.status, HealthStatus::Good);
    assert_eq!(report.life_used_percent, Some(6));
    assert_eq!(report.power_on_hours, Some(5487));
    assert!(
        report
            .temperature_celsius
            .is_some_and(|celsius| (celsius - 41.85).abs() < 0.01)
    );
}

#[test]
fn desktop_capabilities_follow_the_manager() {
    let capabilities = fixtures::desktop().capabilities;
    assert!(capabilities.encryption);
    let exfat = capabilities
        .formats
        .iter()
        .find(|format| format.filesystem == FilesystemKind::Exfat)
        .unwrap();
    assert_eq!(exfat.missing_package.as_deref(), Some("exfatprogs"));
    let ext4 = capabilities
        .formats
        .iter()
        .find(|format| format.filesystem == FilesystemKind::Ext4)
        .unwrap();
    assert_eq!(ext4.missing_package, None);
    let inventory = fixtures::desktop();
    assert!(inventory.resize_modes(&FilesystemKind::Ext4).online_grow);
    assert!(!inventory.resize_modes(&FilesystemKind::Btrfs).online_grow);
}

#[test]
fn usb_stick_is_removable_and_mounted() {
    let inventory = fixtures::removable();
    let stick = disk(&inventory, "/dev/sda");
    assert_eq!(stick.kind, DiskKind::Usb);
    assert_eq!(stick.name, "SanDisk Ultra");
    assert_eq!(stick.table, Some(TableKind::Mbr));
    assert_eq!(
        stick.removal,
        Removal::Removable {
            ejectable: false,
            can_power_off: true
        }
    );
    assert_eq!(stick.health, Health::Unavailable);
    assert_eq!(stick.rotation_rate, None);
    assert!(
        inventory.disks.iter().all(|disk| disk.device != "/dev/sdf"),
        "card readers without media are hidden"
    );
    let volume = stick
        .partitions()
        .next()
        .and_then(|partition| partition.content.volume())
        .unwrap();
    assert_eq!(volume.filesystem, FilesystemKind::Exfat);
    assert_eq!(volume.label, "SANDISK");
    assert_eq!(
        volume.mount_points,
        [Path::new("/run/media/marvelxcodes/SANDISK")]
    );
}

#[test]
fn user_iso_is_an_image_with_a_whole_disk_filesystem() {
    let inventory = fixtures::removable();
    let images: Vec<_> = inventory
        .disks
        .iter()
        .filter(|disk| disk.kind == DiskKind::Image)
        .collect();
    assert_eq!(
        images.len(),
        1,
        "root's Waydroid loops and unbacked loops stay hidden"
    );
    let iso = images[0];
    assert_eq!(iso.name, "crownos-2026.11-x86_64.iso");
    assert_eq!(iso.id.object_path(), iso.block.object_path());
    assert_eq!(
        iso.backing_file.as_deref(),
        Some(Path::new(
            "/home/marvelxcodes/Downloads/crownos-2026.11-x86_64.iso"
        ))
    );
    assert_eq!(iso.table, None);
    assert_eq!(iso.layout.len(), 1);
    let content = iso.whole_disk_content().unwrap();
    assert!(content.is_whole_disk());
    assert_eq!(content.block, iso.block);
    assert_eq!((content.offset, content.size), (ByteSize(0), iso.size));
    assert_eq!(
        content.content.volume().map(|volume| &volume.filesystem),
        Some(&FilesystemKind::Other("iso9660".into()))
    );
}

#[test]
fn images_set_up_by_another_user_are_hidden() {
    let map = fixtures::merged_object_map(&[json::ISO_IMAGE]);
    assert!(
        build(&map, 1001)
            .disks
            .iter()
            .all(|disk| disk.kind != DiskKind::Image)
    );
    assert_eq!(
        build(&map, FIXTURE_UID)
            .disks
            .iter()
            .filter(|disk| disk.kind == DiskKind::Image)
            .count(),
        1
    );
}

#[test]
fn luks_partitions_resolve_locked_and_unlocked() {
    let inventory = fixtures::encrypted();
    let external = disk(&inventory, "/dev/sdd");
    assert_eq!(external.kind, DiskKind::Usb);
    assert_eq!(external.name, "Samsung PSSD T7");
    let [locked, unlocked] = [0, 1].map(|index| &external.partitions().nth(index).unwrap().content);
    assert_eq!(
        locked,
        &Content::Encrypted(Encryption {
            version: LuksVersion::Luks2,
            header_size: ByteSize(16 * MIB),
            unlocked: None
        })
    );
    let Content::Encrypted(Encryption {
        unlocked: Some(cleartext),
        ..
    }) = unlocked
    else {
        panic!("expected an unlocked LUKS volume");
    };
    assert_eq!(
        cleartext.cleartext.object_path(),
        "/org/freedesktop/UDisks2/block_devices/dm_2d0"
    );
    let volume = unlocked.volume().unwrap();
    assert_eq!(
        (volume.filesystem.clone(), volume.label.as_str()),
        (FilesystemKind::Ext4, "Photos")
    );
    assert_eq!(
        volume.usage.map(|usage| usage.used),
        Some(ByteSize(210_000_000_000))
    );
    assert!(
        inventory
            .disks
            .iter()
            .all(|disk| !disk.device.starts_with("/dev/dm")),
        "cleartext devices aren't disks"
    );
}

#[test]
fn sata_drives_classify_by_rotation_and_report_ata_health() {
    let inventory = fixtures::ata_failing();
    let ssd = disk(&inventory, "/dev/sdb");
    let hdd = disk(&inventory, "/dev/sdc");
    assert_eq!((ssd.kind, hdd.kind), (DiskKind::Ssd, DiskKind::Hdd));
    assert_eq!(hdd.rotation_rate, Some(7200));
    assert!(matches!(&ssd.health, Health::Reported(report) if report.status == HealthStatus::Good));
    let Health::Reported(report) = &hdd.health else {
        panic!("expected ATA health");
    };
    assert_eq!(report.status, HealthStatus::Failing);
    assert_eq!(report.bad_sectors, Some(24));
    assert_eq!(
        report.notes,
        [
            HealthNote::SmartFailing,
            HealthNote::BadSectors,
            HealthNote::AttributesFailing
        ]
    );
}

#[test]
fn gpt_free_space_gaps_are_listed_in_offset_order() {
    let inventory = fixtures::ata_failing();
    let ssd = disk(&inventory, "/dev/sdb");
    let data_end = MIB + 100 * GIB;
    let swap_end = data_end + 50 * GIB + 200 * GIB + 8 * GIB;
    let usable_end = (ssd.size.0 - MIB) / MIB * MIB;
    assert_eq!(
        summary(ssd),
        [
            "/dev/sdb1 Some(Ext4)".to_owned(),
            format!("free {data_end}..{}", data_end + 50 * GIB),
            "/dev/sdb2 Some(Btrfs)".to_owned(),
            "/dev/sdb3 None".to_owned(),
            format!("free {swap_end}..{usable_end}"),
        ]
    );
    let swap = &ssd.partitions().nth(2).unwrap().content;
    assert_eq!(swap, &Content::Swap { active: true });
}

#[test]
fn mbr_extended_container_is_hidden_and_logical_members_shown() {
    let inventory = fixtures::ata_failing();
    let hdd = disk(&inventory, "/dev/sdc");
    assert_eq!(hdd.table, Some(TableKind::Mbr));
    let contents: Vec<_> = hdd
        .partitions()
        .map(|partition| (partition.number, &partition.content))
        .collect();
    assert_eq!(contents.len(), 2);
    assert!(
        matches!(contents[0], (1, Content::Filesystem(volume)) if volume.filesystem == FilesystemKind::Ntfs)
    );
    assert_eq!(
        contents[1],
        (
            5,
            &Content::Member {
                kind: "LVM2_member".into()
            }
        )
    );
    let free: Vec<_> = hdd
        .layout
        .iter()
        .filter(|region| matches!(region, Region::Free { .. }))
        .collect();
    assert_eq!(
        free.len(),
        1,
        "only the space after the extended partition is free"
    );
    assert_eq!(free[0].offset(), ByteSize(MIB + 900 * GIB));
}

#[test]
fn blank_disk_is_one_free_region_without_a_table() {
    let inventory = fixtures::blank_disk();
    let blank = disk(&inventory, "/dev/sde");
    assert_eq!(blank.kind, DiskKind::Hdd);
    assert_eq!(blank.table, None);
    assert_eq!(
        blank.layout,
        [Region::Free {
            offset: ByteSize(0),
            size: blank.size
        }]
    );
    assert_eq!(blank.whole_disk_content(), None);
}

#[test]
fn drives_sort_by_udisks_sort_key_then_images_last() {
    let mut map = fixtures::merged_object_map(&[json::USB_STICK, json::ISO_IMAGE, json::ATA]);
    map.objects.retain(|path, _| !path.contains("nvme"));
    let devices: Vec<_> = build(&map, FIXTURE_UID)
        .disks
        .into_iter()
        .map(|disk| disk.device)
        .collect();
    assert_eq!(devices, ["/dev/sdb", "/dev/sdc", "/dev/sda", "/dev/loop0"]);
}
