use super::resize::{Direction, ResizeMode, plan};
use super::*;
use crate::guard::test_inventory::*;
use crate::model::{Content, FilesystemKind};

fn bounds(inventory: &Inventory, path: &str) -> Result<ResizeBounds, Refusal> {
    resize_bounds(inventory, &BlockId::new(path))
}

fn filesystem(kind: FilesystemKind, mount_point: Option<&str>, used: Option<ByteSize>) -> Content {
    Content::Filesystem(volume(kind, "data", mount_point, used))
}

#[test]
fn unmounted_ntfs_shrinks_to_used_plus_margin_and_grows_into_free_space() {
    let inventory = desktop_with_usb();
    let windows = bounds(&inventory, WINDOWS_BLOCK).expect("windows resizable");
    assert_eq!(
        windows,
        ResizeBounds {
            min: gib(22),
            current: gib(50),
            max: gib(150)
        }
    );
}

#[test]
fn margin_is_ten_percent_above_ten_gib() {
    let inventory = single_partition(
        filesystem(FilesystemKind::Ext4, None, Some(gib(40))),
        gib(100),
        gib(0),
    );
    assert_eq!(
        bounds(&inventory, USB_BLOCK).expect("shrinkable").min,
        gib(44)
    );
}

#[test]
fn minimum_rounds_up_to_mebibytes_and_never_exceeds_current() {
    let used = ByteSize(gib(1).0 + 1);
    let inventory = single_partition(
        filesystem(FilesystemKind::Ext4, None, Some(used)),
        gib(4),
        gib(1),
    );
    assert_eq!(
        bounds(&inventory, USB_BLOCK).expect("shrinkable").min,
        ByteSize(gib(2).0 + mib(1).0)
    );

    let inventory = single_partition(
        filesystem(FilesystemKind::Ext4, None, Some(gib(3))),
        gib(4),
        gib(1),
    );
    let nearly_full = bounds(&inventory, USB_BLOCK).expect("growable");
    assert_eq!(nearly_full.min, nearly_full.current);
    assert_eq!(nearly_full.max, gib(5));
}

#[test]
fn encrypted_bounds_include_the_luks_header() {
    let inner = filesystem(
        FilesystemKind::Ext4,
        Some("/run/media/user/data"),
        Some(gib(2)),
    );
    let inventory = single_partition(encrypted(USB_CLEARTEXT, Some(inner)), gib(10), gib(10));
    let expected = ResizeBounds {
        min: ByteSize(gib(3).0 + LUKS_HEADER.0),
        current: gib(10),
        max: gib(20),
    };
    assert_eq!(bounds(&inventory, USB_BLOCK), Ok(expected));
    assert_eq!(bounds(&inventory, USB_CLEARTEXT), Ok(expected));
}

#[test]
fn unknown_usage_prevents_shrinking() {
    let inventory = single_partition(
        filesystem(FilesystemKind::Ext4, None, None),
        gib(10),
        gib(10),
    );
    assert_eq!(
        bounds(&inventory, USB_BLOCK),
        Ok(ResizeBounds {
            min: gib(10),
            current: gib(10),
            max: gib(20)
        })
    );
}

#[test]
fn no_free_space_and_nothing_to_shrink_means_no_room() {
    let inventory = single_partition(
        filesystem(FilesystemKind::Xfs, None, Some(gib(1))),
        gib(10),
        gib(0),
    );
    assert_eq!(bounds(&inventory, USB_BLOCK), Err(Refusal::NoRoom));
}

#[test]
fn xfs_never_shrinks() {
    let inventory = single_partition(
        filesystem(FilesystemKind::Xfs, Some("/srv"), Some(gib(1))),
        gib(10),
        gib(5),
    );
    let xfs = plan(&inventory, &BlockId::new(USB_BLOCK)).expect("xfs grows");
    assert_eq!(xfs.layers.mode(Direction::Shrink), None);
    assert_eq!(xfs.layers.mode(Direction::Grow), Some(ResizeMode::Online));
    assert_eq!(xfs.bounds.min, gib(10));
}

#[test]
fn ext4_shrinks_offline_and_grows_online() {
    let mounted = single_partition(
        filesystem(FilesystemKind::Ext4, Some("/data"), Some(gib(1))),
        gib(10),
        gib(5),
    );
    let mounted = plan(&mounted, &BlockId::new(USB_BLOCK)).expect("ext4 resizable");
    assert_eq!(
        mounted.layers.mode(Direction::Shrink),
        Some(ResizeMode::Offline)
    );
    assert_eq!(
        mounted.layers.mode(Direction::Grow),
        Some(ResizeMode::Online)
    );

    let unmounted = single_partition(
        filesystem(FilesystemKind::Ext4, None, Some(gib(1))),
        gib(10),
        gib(5),
    );
    let unmounted = plan(&unmounted, &BlockId::new(USB_BLOCK)).expect("ext4 resizable");
    assert_eq!(
        unmounted.layers.mode(Direction::Shrink),
        Some(ResizeMode::Offline)
    );
    assert_eq!(
        unmounted.layers.mode(Direction::Grow),
        Some(ResizeMode::Offline)
    );
}

#[test]
fn btrfs_is_resized_online_even_when_unmounted() {
    let inventory = single_partition(
        filesystem(FilesystemKind::Btrfs, None, Some(gib(1))),
        gib(10),
        gib(5),
    );
    let btrfs = plan(&inventory, &BlockId::new(USB_BLOCK)).expect("btrfs resizable");
    assert_eq!(
        btrfs.layers.mode(Direction::Shrink),
        Some(ResizeMode::Online)
    );
    assert_eq!(btrfs.layers.mode(Direction::Grow), Some(ResizeMode::Online));
}

#[test]
fn system_volumes_only_resize_online() {
    let inventory = desktop_with_usb();
    let root = plan(&inventory, &BlockId::new(ROOT_BLOCK));
    assert_eq!(root.err(), Some(Refusal::NoRoom));

    let fat_root = single_partition(
        filesystem(FilesystemKind::Fat32, Some("/boot"), Some(gib(1))),
        gib(10),
        gib(5),
    );
    assert_eq!(
        bounds(&fat_root, USB_BLOCK),
        Err(Refusal::SystemVolume {
            name: "data".to_owned()
        })
    );
}

#[test]
fn unsupported_filesystems_are_refused() {
    let unsupported =
        |content: Content| bounds(&single_partition(content, gib(10), gib(5)), USB_BLOCK);
    let named = |filesystem: &str| {
        Err(Refusal::Unsupported {
            filesystem: filesystem.to_owned(),
        })
    };
    assert_eq!(
        unsupported(filesystem(FilesystemKind::Exfat, None, Some(gib(1)))),
        named("exFAT")
    );
    assert_eq!(
        unsupported(filesystem(FilesystemKind::Other("udf".into()), None, None)),
        named("udf")
    );
    assert_eq!(unsupported(Content::Swap { active: false }), named("Swap"));
    let system = Err(Refusal::SystemVolume {
        name: "data".to_owned(),
    });
    assert_eq!(
        unsupported(filesystem(
            FilesystemKind::F2fs,
            Some("/home"),
            Some(gib(1))
        )),
        system
    );
    assert_eq!(unsupported(Content::Empty), Err(Refusal::NotApplicable));
}

#[test]
fn f2fs_grows_only_while_unmounted() {
    let inventory = single_partition(
        filesystem(FilesystemKind::F2fs, None, Some(gib(1))),
        gib(10),
        gib(5),
    );
    assert_eq!(
        bounds(&inventory, USB_BLOCK),
        Ok(ResizeBounds {
            min: gib(10),
            current: gib(10),
            max: gib(15)
        })
    );
}

#[test]
fn missing_blocks_are_not_applicable() {
    assert_eq!(
        bounds(&desktop_with_usb(), "/missing"),
        Err(Refusal::NotApplicable)
    );
}
