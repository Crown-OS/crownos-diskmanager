use std::path::PathBuf;

use crate::model::{
    BlockId, ByteSize, Capabilities, Content, Disk, DiskId, DiskKind, Encryption, FilesystemKind,
    FormatSupport, Health, Inventory, LuksVersion, Partition, Region, Removal, ResizeModes,
    TableKind, Unlocked, Usage, Volume,
};

pub const NVME_DISK: &str = "/org/freedesktop/UDisks2/drives/Samsung_980";
pub const NVME_BLOCK: &str = "/org/freedesktop/UDisks2/block_devices/nvme0n1";
pub const EFI_BLOCK: &str = "/org/freedesktop/UDisks2/block_devices/nvme0n1p1";
pub const ROOT_BLOCK: &str = "/org/freedesktop/UDisks2/block_devices/nvme0n1p2";
pub const WINDOWS_BLOCK: &str = "/org/freedesktop/UDisks2/block_devices/nvme0n1p3";
pub const SWAP_BLOCK: &str = "/org/freedesktop/UDisks2/block_devices/nvme0n1p4";
pub const USB_DISK: &str = "/org/freedesktop/UDisks2/drives/SanDisk_Ultra";
pub const PHOTOS_BLOCK: &str = "/org/freedesktop/UDisks2/block_devices/sdb1";
pub const USB_BLOCK: &str = "/org/freedesktop/UDisks2/block_devices/sdb2";
pub const USB_CLEARTEXT: &str = "/org/freedesktop/UDisks2/block_devices/dm_2d0";

pub const LUKS_HEADER: ByteSize = ByteSize(16 << 20);
const GB: u64 = 1_000_000_000;

pub fn mib(count: u64) -> ByteSize {
    ByteSize(count << 20)
}

pub fn gib(count: u64) -> ByteSize {
    ByteSize(count << 30)
}

pub fn volume(
    filesystem: FilesystemKind,
    label: &str,
    mount_point: Option<&str>,
    used: Option<ByteSize>,
) -> Volume {
    Volume {
        filesystem,
        label: label.to_owned(),
        uuid: format!("{label}-uuid"),
        mount_points: mount_point.map(PathBuf::from).into_iter().collect(),
        usage: used.map(|used| Usage {
            total: ByteSize(used.0 * 2),
            used,
        }),
    }
}

pub fn filesystem_mounted_at(mount_point: &str) -> Content {
    Content::Filesystem(volume(
        FilesystemKind::Ext4,
        "data",
        Some(mount_point),
        None,
    ))
}

pub fn encrypted(cleartext: &str, inner: Option<Content>) -> Content {
    Content::Encrypted(Encryption {
        version: LuksVersion::Luks2,
        header_size: LUKS_HEADER,
        unlocked: inner.map(|content| {
            Box::new(Unlocked {
                cleartext: BlockId::new(cleartext),
                content,
            })
        }),
    })
}

pub fn partition(
    block: &str,
    number: u32,
    offset: ByteSize,
    size: ByteSize,
    name: &str,
    content: Content,
) -> Region {
    Region::Partition(Partition {
        block: BlockId::new(block),
        device: format!("/dev/{}", block.rsplit('/').next().unwrap_or(block)),
        number,
        offset,
        size,
        name: name.to_owned(),
        type_id: "0fc63daf-8483-4772-8e79-3d69d8477de4".to_owned(),
        uuid: format!("uuid-{number}-{}", offset.0),
        content,
    })
}

pub fn free(offset: ByteSize, size: ByteSize) -> Region {
    Region::Free { offset, size }
}

pub fn disk(
    id: &str,
    block: &str,
    kind: DiskKind,
    removal: Removal,
    size: ByteSize,
    layout: Vec<Region>,
) -> Disk {
    Disk {
        id: DiskId::new(id),
        block: BlockId::new(block),
        device: format!("/dev/{}", block.rsplit('/').next().unwrap_or(block)),
        kind,
        name: id.rsplit('/').next().unwrap_or(id).replace('_', " "),
        vendor: String::new(),
        model: String::new(),
        serial: format!("serial-{}", size.0),
        revision: String::new(),
        size,
        connection: String::new(),
        rotation_rate: None,
        removal,
        table: Some(TableKind::Gpt),
        table_uuid: format!("table-{}", size.0),
        layout,
        health: Health::Unavailable,
        backing_file: None,
    }
}

pub const REMOVABLE: Removal = Removal::Removable {
    ejectable: true,
    can_power_off: true,
};

pub fn desktop_disk() -> Disk {
    let efi = mib(1);
    let root = efi.0 + mib(512).0;
    let windows = root + gib(100).0;
    let gap = windows + gib(50).0;
    let swap = gap + gib(100).0;
    disk(
        NVME_DISK,
        NVME_BLOCK,
        DiskKind::Nvme,
        Removal::Fixed,
        ByteSize(swap + gib(8).0 + mib(1).0),
        vec![
            partition(
                EFI_BLOCK,
                1,
                efi,
                mib(512),
                "EFI system partition",
                Content::Filesystem(volume(
                    FilesystemKind::Fat32,
                    "",
                    Some("/boot/efi"),
                    Some(mib(100)),
                )),
            ),
            partition(
                ROOT_BLOCK,
                2,
                ByteSize(root),
                gib(100),
                "",
                Content::Filesystem(volume(
                    FilesystemKind::Ext4,
                    "root",
                    Some("/"),
                    Some(gib(40)),
                )),
            ),
            partition(
                WINDOWS_BLOCK,
                3,
                ByteSize(windows),
                gib(50),
                "Basic data partition",
                Content::Filesystem(volume(FilesystemKind::Ntfs, "Windows", None, Some(gib(20)))),
            ),
            free(ByteSize(gap), gib(100)),
            partition(
                SWAP_BLOCK,
                4,
                ByteSize(swap),
                gib(8),
                "",
                Content::Swap { active: true },
            ),
        ],
    )
}

pub fn usb_disk() -> Disk {
    let photos = mib(1);
    let secure = photos.0 + 30 * GB;
    let gap = secure + 20 * GB;
    let mut usb = disk(
        USB_DISK,
        "/org/freedesktop/UDisks2/block_devices/sdb",
        DiskKind::Usb,
        REMOVABLE,
        ByteSize(64 * GB),
        vec![
            partition(
                PHOTOS_BLOCK,
                1,
                photos,
                ByteSize(30 * GB),
                "",
                Content::Filesystem(volume(
                    FilesystemKind::Exfat,
                    "Photos",
                    Some("/run/media/user/Photos"),
                    Some(ByteSize(10 * GB)),
                )),
            ),
            partition(
                USB_BLOCK,
                2,
                ByteSize(secure),
                ByteSize(20 * GB),
                "",
                encrypted(
                    USB_CLEARTEXT,
                    Some(Content::Filesystem(volume(
                        FilesystemKind::Ext4,
                        "Secure",
                        Some("/run/media/user/Secure"),
                        Some(ByteSize(5 * GB)),
                    ))),
                ),
            ),
            free(ByteSize(gap), ByteSize(64 * GB - gap)),
        ],
    );
    usb.table = Some(TableKind::Mbr);
    usb
}

pub fn live_medium() -> Disk {
    disk(
        "/org/freedesktop/UDisks2/drives/Live_USB",
        "/org/freedesktop/UDisks2/block_devices/sdc",
        DiskKind::Usb,
        REMOVABLE,
        ByteSize(16 * GB),
        vec![partition(
            "/org/freedesktop/UDisks2/block_devices/sdc1",
            1,
            ByteSize(0),
            ByteSize(2 * GB),
            "",
            Content::Filesystem(volume(
                FilesystemKind::Other("iso9660".into()),
                "CROWNOS",
                Some("/run/archiso/bootmnt"),
                None,
            )),
        )],
    )
}

pub fn capabilities() -> Capabilities {
    let offline = ResizeModes {
        offline_shrink: true,
        offline_grow: true,
        ..ResizeModes::default()
    };
    Capabilities {
        formats: FilesystemKind::FORMATTABLE
            .iter()
            .map(|filesystem| FormatSupport {
                filesystem: filesystem.clone(),
                missing_package: (filesystem == &FilesystemKind::F2fs)
                    .then(|| "f2fs-tools".to_owned()),
            })
            .collect(),
        resize: vec![
            (
                FilesystemKind::Ext4,
                ResizeModes {
                    online_grow: true,
                    ..offline
                },
            ),
            (
                FilesystemKind::Xfs,
                ResizeModes {
                    online_grow: true,
                    ..ResizeModes::default()
                },
            ),
            (
                FilesystemKind::Btrfs,
                ResizeModes {
                    online_shrink: true,
                    online_grow: true,
                    ..ResizeModes::default()
                },
            ),
            (FilesystemKind::Fat32, offline),
            (FilesystemKind::Ntfs, offline),
            (
                FilesystemKind::F2fs,
                ResizeModes {
                    offline_grow: true,
                    ..ResizeModes::default()
                },
            ),
        ],
        encryption: true,
    }
}

pub fn desktop_with_usb() -> Inventory {
    Inventory {
        disks: vec![desktop_disk(), usb_disk()],
        capabilities: capabilities(),
    }
}

pub fn with_disk(disk: Disk) -> Inventory {
    Inventory {
        disks: vec![disk],
        capabilities: capabilities(),
    }
}

/// A removable disk holding one partition with `content`, followed by `free_after` bytes of free space.
pub fn single_partition(content: Content, size: ByteSize, free_after: ByteSize) -> Inventory {
    let offset = mib(1);
    let mut layout = vec![partition(USB_BLOCK, 1, offset, size, "", content)];
    if free_after.0 > 0 {
        layout.push(free(ByteSize(offset.0 + size.0), free_after));
    }
    with_disk(disk(
        USB_DISK,
        "/org/freedesktop/UDisks2/block_devices/sdb",
        DiskKind::Usb,
        REMOVABLE,
        ByteSize(offset.0 + size.0 + free_after.0),
        layout,
    ))
}
