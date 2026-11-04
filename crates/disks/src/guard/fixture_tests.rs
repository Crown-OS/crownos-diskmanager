use super::*;
use crate::fixtures;
use crate::limits::{ResizeBounds, default_filesystem, resize_bounds};
use crate::model::{BlockId, Disk, DiskId, FilesystemKind};

const BLOCKS: &str = "/org/freedesktop/UDisks2/block_devices/";
const SAMSUNG_NVME: &str =
    "/org/freedesktop/UDisks2/drives/SAMSUNG_MZVL2512HDJD_00BL2_S6Z5NE0W620612";
const CRUCIAL_NVME: &str = "/org/freedesktop/UDisks2/drives/CT500P3PSSD8_25124F20B4F1";
const SANDISK: &str = "/org/freedesktop/UDisks2/drives/SanDisk_Ultra_4C530001230405117053";
const T7: &str = "/org/freedesktop/UDisks2/drives/Samsung_PSSD_T7_S5SXNS0R700123K";
const SATA_SSD: &str = "/org/freedesktop/UDisks2/drives/Samsung_SSD_870_EVO_500GB_S6PXNM0T123456A";
const SATA_HDD: &str = "/org/freedesktop/UDisks2/drives/ST1000DM010_2EP102_Z9A1B2C3";
const BLANK_HDD: &str = "/org/freedesktop/UDisks2/drives/WDC_WD20EZRZ_00Z5HB0_WD_WCC4M1234567";

fn block(name: &str) -> Target {
    Target::Block(BlockId::new(format!("{BLOCKS}{name}")))
}

fn disk(path: &str) -> Target {
    Target::Disk(DiskId::new(path))
}

fn refused(inventory: &Inventory, target: &Target, action: Action) -> Option<Refusal> {
    match availability(inventory, target, action) {
        Availability::Available => None,
        Availability::Unavailable(refusal) => Some(refusal),
    }
}

fn bounds(inventory: &Inventory, name: &str) -> Result<ResizeBounds, Refusal> {
    resize_bounds(inventory, &BlockId::new(format!("{BLOCKS}{name}")))
}

fn find_disk<'a>(inventory: &'a Inventory, path: &str) -> &'a Disk {
    inventory.disk(&DiskId::new(path)).expect("fixture disk")
}

#[test]
fn desktop_nvme_with_root_and_boot_is_the_system_disk() {
    let inventory = fixtures::desktop();
    let system = disk(SAMSUNG_NVME);
    [Action::EraseDisk, Action::RestoreImage, Action::Eject]
        .iter()
        .for_each(|action| {
            assert_eq!(
                refused(&inventory, &system, *action),
                Some(Refusal::SystemDisk),
                "{action:?}"
            );
        });
    ["nvme1n1p5", "nvme1n1p6"].iter().for_each(|partition| {
        [
            Action::Format,
            Action::DeletePartition,
            Action::Unmount,
            Action::Check,
        ]
        .iter()
        .for_each(|action| {
            assert!(
                matches!(
                    refused(&inventory, &block(partition), *action),
                    Some(Refusal::SystemVolume { .. })
                ),
                "{partition} {action:?}"
            );
        });
    });
    assert_eq!(
        refused(&inventory, &block("nvme1n1p6"), Action::Rename),
        None
    );
    assert_eq!(bounds(&inventory, "nvme1n1p6"), Err(Refusal::NoRoom));
}

#[test]
fn desktop_workspace_disk_is_internal_but_not_system() {
    let inventory = fixtures::desktop();
    let workspace = disk(CRUCIAL_NVME);
    assert_eq!(refused(&inventory, &workspace, Action::EraseDisk), None);
    assert_eq!(
        confirmation_tier(&inventory, &workspace, Action::EraseDisk),
        ConfirmationTier::TypeName
    );
    assert_eq!(
        refused(&inventory, &workspace, Action::Eject),
        Some(Refusal::NotApplicable)
    );
    assert_eq!(bounds(&inventory, "nvme0n1p3"), Err(Refusal::NoRoom));
    assert_eq!(
        default_filesystem(find_disk(&inventory, CRUCIAL_NVME)),
        FilesystemKind::Ext4
    );
}

#[test]
fn usb_stick_and_iso_image() {
    let inventory = fixtures::removable();
    let stick = disk(SANDISK);
    [
        Action::EraseDisk,
        Action::RestoreImage,
        Action::Eject,
        Action::CreateImage,
    ]
    .iter()
    .for_each(|action| {
        assert_eq!(refused(&inventory, &stick, *action), None, "{action:?}");
    });
    assert_eq!(
        confirmation_tier(&inventory, &stick, Action::EraseDisk),
        ConfirmationTier::Destructive
    );
    assert_eq!(
        default_filesystem(find_disk(&inventory, SANDISK)),
        FilesystemKind::Exfat
    );
    assert_eq!(
        bounds(&inventory, "sda1"),
        Err(Refusal::Unsupported {
            filesystem: "exFAT".to_owned()
        })
    );
    assert_eq!(refused(&inventory, &block("sda1"), Action::Format), None);
    assert_eq!(
        losses(&inventory, &stick),
        [Loss {
            name: "SANDISK".to_owned(),
            filesystem: Some(FilesystemKind::Exfat),
            used: Some(ByteSize(12_000_000_000))
        }]
    );

    let iso = disk(&format!("{BLOCKS}loop0"));
    let iso_volume = block("loop0");
    assert_eq!(refused(&inventory, &iso, Action::Eject), None);
    assert_eq!(
        refused(&inventory, &iso, Action::CreatePartition),
        Some(Refusal::NoTable)
    );
    assert_eq!(refused(&inventory, &iso_volume, Action::Unmount), None);
    assert_eq!(
        refused(&inventory, &iso_volume, Action::DeletePartition),
        Some(Refusal::NotApplicable)
    );
    assert_eq!(
        refused(&inventory, &iso_volume, Action::Resize),
        Some(Refusal::NotApplicable)
    );
    assert_eq!(
        refused(&inventory, &iso_volume, Action::Rename),
        Some(Refusal::Unsupported {
            filesystem: "iso9660".to_owned()
        })
    );
    assert_eq!(
        confirmation_tier(&inventory, &iso, Action::RestoreImage),
        ConfirmationTier::Destructive
    );
    assert!(fingerprint(&inventory, &iso_volume).is_some());
}

#[test]
fn encrypted_usb_ssd() {
    let inventory = fixtures::encrypted();
    let vault = block("sdd1");
    assert_eq!(
        refused(&inventory, &vault, Action::Mount),
        Some(Refusal::Locked {
            name: "Vault".to_owned()
        })
    );
    assert_eq!(refused(&inventory, &vault, Action::Unlock), None);
    assert_eq!(refused(&inventory, &vault, Action::Format), None);

    let photos = block("sdd2");
    assert_eq!(refused(&inventory, &photos, Action::Lock), None);
    assert_eq!(refused(&inventory, &block("dm_2d0"), Action::Unmount), None);
    let photos_bounds = bounds(&inventory, "sdd2").expect("encrypted ext4 resizes");
    assert_eq!(photos_bounds.min, ByteSize(231_017_021_440));
    assert_eq!(
        photos_bounds.max,
        ByteSize(644_245_094_400 + 33_834_401_792)
    );

    let lost: Vec<_> = losses(&inventory, &disk(T7))
        .into_iter()
        .map(|loss| (loss.name, loss.used))
        .collect();
    assert_eq!(
        lost,
        [
            ("Vault".to_owned(), None),
            ("Photos".to_owned(), Some(ByteSize(210_000_000_000)))
        ]
    );
}

#[test]
fn sata_disks_with_swap_lvm_and_unknown_usage() {
    let inventory = fixtures::ata_failing();
    assert_eq!(
        refused(&inventory, &disk(SATA_SSD), Action::EraseDisk),
        Some(Refusal::SystemDisk)
    );
    assert!(matches!(
        refused(&inventory, &block("sdb3"), Action::Format),
        Some(Refusal::SystemVolume { .. })
    ));
    assert_eq!(
        refused(&inventory, &disk(SATA_SSD), Action::CreatePartition),
        None
    );

    let data = bounds(&inventory, "sdb1").expect("unmounted ext4 grows");
    assert_eq!(data.min, data.current);
    assert_eq!(data.max, ByteSize(data.current.0 + 53_687_091_200));
    assert_eq!(
        bounds(&inventory, "sdb2"),
        Err(Refusal::Unsupported {
            filesystem: "Btrfs".to_owned()
        })
    );
    assert_eq!(
        bounds(&inventory, "sdc1"),
        Err(Refusal::Unsupported {
            filesystem: "NTFS".to_owned()
        })
    );

    assert_eq!(
        refused(&inventory, &disk(SATA_HDD), Action::EraseDisk),
        None
    );
    assert_eq!(refused(&inventory, &block("sdc5"), Action::Format), None);
    assert_eq!(losses(&inventory, &disk(SATA_HDD)).len(), 2);
}

#[test]
fn blank_disk_must_be_erased_before_partitioning() {
    let inventory = fixtures::blank_disk();
    let blank = disk(BLANK_HDD);
    assert_eq!(
        refused(&inventory, &blank, Action::CreatePartition),
        Some(Refusal::NoTable)
    );
    assert_eq!(refused(&inventory, &blank, Action::EraseDisk), None);
    assert_eq!(
        confirmation_tier(&inventory, &blank, Action::EraseDisk),
        ConfirmationTier::TypeName
    );
    assert!(losses(&inventory, &blank).is_empty());
}
