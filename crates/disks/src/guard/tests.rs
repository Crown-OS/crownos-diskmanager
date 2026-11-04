use super::test_inventory::*;
use super::*;
use crate::model::{BlockId, Content, DiskId, DiskKind, Partition, Region};

fn block(path: &str) -> Target {
    Target::Block(BlockId::new(path))
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

fn system_volume(name: &str) -> Option<Refusal> {
    Some(Refusal::SystemVolume {
        name: name.to_owned(),
    })
}

const DISK_ACTIONS: [Action; 5] = [
    Action::Eject,
    Action::EraseDisk,
    Action::RestoreImage,
    Action::CreatePartition,
    Action::CreateImage,
];
const BLOCK_ONLY_ACTIONS: [Action; 10] = [
    Action::Mount,
    Action::Unmount,
    Action::Rename,
    Action::Unlock,
    Action::Lock,
    Action::Check,
    Action::Repair,
    Action::Format,
    Action::DeletePartition,
    Action::Resize,
];

#[test]
fn system_disk_refuses_whole_disk_changes_but_allows_new_partitions() {
    let inventory = desktop_with_usb();
    let nvme = disk(NVME_DISK);
    assert_eq!(
        refused(&inventory, &nvme, Action::EraseDisk),
        Some(Refusal::SystemDisk)
    );
    assert_eq!(
        refused(&inventory, &nvme, Action::RestoreImage),
        Some(Refusal::SystemDisk)
    );
    assert_eq!(
        refused(&inventory, &nvme, Action::Eject),
        Some(Refusal::SystemDisk)
    );
    assert_eq!(refused(&inventory, &nvme, Action::CreatePartition), None);
    assert_eq!(refused(&inventory, &nvme, Action::CreateImage), None);
}

#[test]
fn block_actions_do_not_apply_to_disks() {
    let inventory = desktop_with_usb();
    BLOCK_ONLY_ACTIONS.iter().for_each(|action| {
        assert_eq!(
            refused(&inventory, &disk(USB_DISK), *action),
            Some(Refusal::NotApplicable),
            "{action:?}"
        );
    });
    [Action::Eject, Action::EraseDisk, Action::CreatePartition]
        .iter()
        .for_each(|action| {
            assert_eq!(
                refused(&inventory, &block(PHOTOS_BLOCK), *action),
                Some(Refusal::NotApplicable),
                "{action:?}"
            );
        });
}

#[test]
fn removable_disk_allows_every_disk_action() {
    let inventory = desktop_with_usb();
    DISK_ACTIONS.iter().for_each(|action| {
        assert_eq!(
            refused(&inventory, &disk(USB_DISK), *action),
            None,
            "{action:?}"
        );
    });
}

#[test]
fn whole_disk_block_path_means_the_disk() {
    let inventory = desktop_with_usb();
    let usb_block = Target::Block(inventory.disks[1].block.clone());
    assert_eq!(refused(&inventory, &usb_block, Action::EraseDisk), None);
}

#[test]
fn unknown_targets_are_not_applicable() {
    let inventory = desktop_with_usb();
    [disk("/gone"), block("/gone")].iter().for_each(|target| {
        assert_eq!(
            refused(&inventory, target, Action::Format),
            Some(Refusal::NotApplicable)
        );
        assert!(fingerprint(&inventory, target).is_none());
        assert!(losses(&inventory, target).is_empty());
    });
}

#[test]
fn live_medium_refuses_every_change() {
    let medium = live_medium();
    let medium_disk = Target::Disk(medium.id.clone());
    let medium_volume = Target::Block(
        medium
            .partitions()
            .next()
            .expect("iso volume")
            .block
            .clone(),
    );
    let inventory = with_disk(medium);
    [
        Action::Eject,
        Action::EraseDisk,
        Action::RestoreImage,
        Action::CreatePartition,
    ]
    .iter()
    .for_each(|action| {
        assert_eq!(
            refused(&inventory, &medium_disk, *action),
            Some(Refusal::LiveMedium),
            "{action:?}"
        );
    });
    [
        Action::Unmount,
        Action::Rename,
        Action::Check,
        Action::Format,
        Action::DeletePartition,
        Action::Resize,
    ]
    .iter()
    .for_each(|action| {
        assert_eq!(
            refused(&inventory, &medium_volume, *action),
            Some(Refusal::LiveMedium),
            "{action:?}"
        );
    });
    assert_eq!(refused(&inventory, &medium_disk, Action::CreateImage), None);
}

#[test]
fn in_use_system_partitions_are_protected() {
    let inventory = desktop_with_usb();
    [
        (ROOT_BLOCK, "root"),
        (EFI_BLOCK, "EFI system partition"),
        (SWAP_BLOCK, "/dev/nvme0n1p4"),
    ]
    .iter()
    .for_each(|(path, name)| {
        [
            Action::Format,
            Action::DeletePartition,
            Action::RestoreImage,
        ]
        .iter()
        .for_each(|action| {
            assert_eq!(
                refused(&inventory, &block(path), *action),
                system_volume(name),
                "{path} {action:?}"
            );
        });
    });
    assert_eq!(
        refused(&inventory, &block(ROOT_BLOCK), Action::Unmount),
        system_volume("root")
    );
    assert_eq!(
        refused(&inventory, &block(ROOT_BLOCK), Action::Check),
        system_volume("root")
    );
    assert_eq!(
        refused(&inventory, &block(ROOT_BLOCK), Action::Rename),
        None
    );
    assert_eq!(
        refused(&inventory, &block(SWAP_BLOCK), Action::Rename),
        None
    );
    assert_eq!(
        refused(&inventory, &block(ROOT_BLOCK), Action::Resize),
        Some(Refusal::NoRoom)
    );
    assert_eq!(
        refused(&inventory, &block(ROOT_BLOCK), Action::CreateImage),
        None
    );
}

#[test]
fn idle_partition_on_system_disk_is_fully_manageable() {
    let inventory = desktop_with_usb();
    let windows = block(WINDOWS_BLOCK);
    [
        Action::Mount,
        Action::Rename,
        Action::Check,
        Action::Repair,
        Action::Format,
        Action::DeletePartition,
        Action::Resize,
        Action::RestoreImage,
    ]
    .iter()
    .for_each(|action| assert_eq!(refused(&inventory, &windows, *action), None, "{action:?}"));
    [Action::Unmount, Action::Unlock, Action::Lock]
        .iter()
        .for_each(|action| {
            assert_eq!(
                refused(&inventory, &windows, *action),
                Some(Refusal::NotApplicable),
                "{action:?}"
            );
        });
}

#[test]
fn mounted_removable_volume_rules() {
    let inventory = desktop_with_usb();
    let photos = block(PHOTOS_BLOCK);
    let mounted = Some(Refusal::Mounted {
        name: "Photos".to_owned(),
    });
    assert_eq!(refused(&inventory, &photos, Action::Rename), mounted);
    assert_eq!(refused(&inventory, &photos, Action::Check), mounted);
    assert_eq!(
        refused(&inventory, &photos, Action::Mount),
        Some(Refusal::NotApplicable)
    );
    assert_eq!(refused(&inventory, &photos, Action::Unmount), None);
    assert_eq!(refused(&inventory, &photos, Action::Format), None);
    assert_eq!(refused(&inventory, &photos, Action::DeletePartition), None);
    assert_eq!(
        refused(&inventory, &photos, Action::Resize),
        Some(Refusal::Unsupported {
            filesystem: "exFAT".to_owned()
        })
    );
}

#[test]
fn unlocked_encryption_is_reachable_from_both_layers() {
    let inventory = desktop_with_usb();
    [block(USB_BLOCK), block(USB_CLEARTEXT)]
        .iter()
        .for_each(|target| {
            assert_eq!(refused(&inventory, target, Action::Unmount), None);
            assert_eq!(
                refused(&inventory, target, Action::Mount),
                Some(Refusal::NotApplicable)
            );
            assert_eq!(
                refused(&inventory, target, Action::Unlock),
                Some(Refusal::NotApplicable)
            );
            assert_eq!(refused(&inventory, target, Action::Lock), None);
            assert_eq!(refused(&inventory, target, Action::Resize), None);
        });
}

#[test]
fn locked_encryption_asks_to_unlock_first() {
    let inventory = single_partition(encrypted(USB_CLEARTEXT, None), gib(8), gib(8));
    let target = block(USB_BLOCK);
    let locked = Some(Refusal::Locked {
        name: "/dev/sdb2".to_owned(),
    });
    [
        Action::Mount,
        Action::Rename,
        Action::Check,
        Action::Repair,
        Action::Resize,
    ]
    .iter()
    .for_each(|action| {
        assert_eq!(refused(&inventory, &target, *action), locked, "{action:?}");
    });
    assert_eq!(refused(&inventory, &target, Action::Unlock), None);
    assert_eq!(
        refused(&inventory, &target, Action::Lock),
        Some(Refusal::NotApplicable)
    );
    assert_eq!(refused(&inventory, &target, Action::Format), None);
}

#[test]
fn encrypted_system_root_cannot_be_locked() {
    let root = encrypted(USB_CLEARTEXT, Some(filesystem_mounted_at("/")));
    let inventory = single_partition(root, gib(8), gib(0));
    assert_eq!(
        refused(&inventory, &block(USB_BLOCK), Action::Lock),
        system_volume("data")
    );
    assert_eq!(
        refused(&inventory, &block(USB_CLEARTEXT), Action::Format),
        system_volume("data")
    );
    assert_eq!(
        refused(&inventory, &disk(USB_DISK), Action::EraseDisk),
        Some(Refusal::SystemDisk)
    );
}

#[test]
fn partitioning_needs_a_table_and_free_space() {
    let mut full = usb_disk();
    full.layout
        .retain(|region| matches!(region, Region::Partition(_)));
    let inventory = with_disk(full.clone());
    assert_eq!(
        refused(&inventory, &disk(USB_DISK), Action::CreatePartition),
        Some(Refusal::NotApplicable)
    );
    full.table = None;
    let inventory = with_disk(full);
    assert_eq!(
        refused(&inventory, &disk(USB_DISK), Action::CreatePartition),
        Some(Refusal::NoTable)
    );
}

#[test]
fn optical_discs_cannot_be_erased() {
    let mut optical = usb_disk();
    optical.kind = DiskKind::Optical;
    let inventory = with_disk(optical);
    assert_eq!(
        refused(&inventory, &disk(USB_DISK), Action::EraseDisk),
        Some(Refusal::NotApplicable)
    );
    assert_eq!(refused(&inventory, &disk(USB_DISK), Action::Eject), None);
}

#[test]
fn fixed_data_disks_cannot_be_ejected() {
    let mut data = usb_disk();
    data.kind = DiskKind::Hdd;
    data.removal = crate::model::Removal::Fixed;
    let inventory = with_disk(data);
    assert_eq!(
        refused(&inventory, &disk(USB_DISK), Action::Eject),
        Some(Refusal::NotApplicable)
    );
    assert_eq!(
        refused(&inventory, &disk(USB_DISK), Action::EraseDisk),
        None
    );
}

#[test]
fn extended_containers_and_whole_disk_filesystems() {
    let mut extended = usb_disk();
    if let Some(Region::Partition(partition)) = extended.layout.get_mut(0) {
        partition.type_id = "0x05".to_owned();
    }
    let inventory = with_disk(extended);
    assert_eq!(
        refused(&inventory, &block(PHOTOS_BLOCK), Action::Format),
        Some(Refusal::NotApplicable)
    );

    let mut stick = usb_disk();
    let whole = Partition {
        block: stick.block.clone(),
        device: stick.device.clone(),
        offset: ByteSize(0),
        size: stick.size,
        content: Content::Filesystem(volume(
            crate::model::FilesystemKind::Fat32,
            "STICK",
            None,
            None,
        )),
        ..stick.partitions().next().expect("partition").clone()
    };
    stick.layout = vec![Region::Partition(whole)];
    let whole_block = Target::Block(stick.block.clone());
    let inventory = with_disk(stick);
    assert_eq!(refused(&inventory, &whole_block, Action::Format), None);
    assert_eq!(refused(&inventory, &whole_block, Action::Mount), None);
    assert_eq!(
        refused(&inventory, &whole_block, Action::DeletePartition),
        Some(Refusal::NotApplicable)
    );
    assert_eq!(
        refused(&inventory, &whole_block, Action::Resize),
        Some(Refusal::NotApplicable)
    );
}

#[test]
fn confirmation_tiers_match_the_plan() {
    let inventory = desktop_with_usb();
    let usb = disk(USB_DISK);
    let nvme = disk(NVME_DISK);
    [
        Action::Mount,
        Action::Unmount,
        Action::Rename,
        Action::Eject,
        Action::Unlock,
        Action::Lock,
        Action::CreateImage,
    ]
    .iter()
    .for_each(|action| {
        assert_eq!(
            confirmation_tier(&inventory, &usb, *action),
            ConfirmationTier::None
        )
    });
    [
        Action::CreatePartition,
        Action::Resize,
        Action::Check,
        Action::Repair,
    ]
    .iter()
    .for_each(|action| {
        assert_eq!(
            confirmation_tier(&inventory, &usb, *action),
            ConfirmationTier::Simple
        )
    });
    [Action::Format, Action::DeletePartition]
        .iter()
        .for_each(|action| {
            assert_eq!(
                confirmation_tier(&inventory, &block(WINDOWS_BLOCK), *action),
                ConfirmationTier::Destructive
            );
        });
    [Action::EraseDisk, Action::RestoreImage]
        .iter()
        .for_each(|action| {
            assert_eq!(
                confirmation_tier(&inventory, &usb, *action),
                ConfirmationTier::Destructive
            );
            assert_eq!(
                confirmation_tier(&inventory, &nvme, *action),
                ConfirmationTier::TypeName
            );
        });
    assert_eq!(
        confirmation_tier(&inventory, &block(WINDOWS_BLOCK), Action::RestoreImage),
        ConfirmationTier::Destructive
    );
}

#[test]
fn images_and_external_fixed_disks_are_not_internal() {
    let mut image = desktop_disk();
    image.kind = DiskKind::Image;
    let inventory = with_disk(image);
    assert_eq!(
        confirmation_tier(&inventory, &disk(NVME_DISK), Action::EraseDisk),
        ConfirmationTier::Destructive
    );
}

#[test]
fn resize_tier_depends_on_direction() {
    assert_eq!(
        resize_confirmation_tier(gib(10), gib(5)),
        ConfirmationTier::Simple
    );
    assert_eq!(
        resize_confirmation_tier(gib(10), gib(20)),
        ConfirmationTier::None
    );
}

#[test]
fn fingerprint_detects_a_different_disk() {
    let inventory = desktop_with_usb();
    let captured = fingerprint(&inventory, &disk(USB_DISK)).expect("usb fingerprint");
    assert_eq!(
        fingerprint(&inventory, &block(PHOTOS_BLOCK)),
        Some(captured.clone())
    );
    assert_eq!(captured.partitions.len(), 2);

    let mut swapped = usb_disk();
    swapped.serial = "another-stick".to_owned();
    assert_ne!(
        Some(captured.clone()),
        fingerprint(&with_disk(swapped), &disk(USB_DISK))
    );

    let mut repartitioned = usb_disk();
    if let Some(Region::Partition(partition)) = repartitioned.layout.get_mut(1) {
        partition.offset = ByteSize(partition.offset.0 + (1 << 20));
    }
    assert_ne!(
        Some(captured),
        fingerprint(&with_disk(repartitioned), &disk(USB_DISK))
    );
}

#[test]
fn image_fingerprint_uses_the_backing_file() {
    let mut image = usb_disk();
    image.kind = DiskKind::Image;
    image.backing_file = Some("/home/user/stick.img".into());
    let captured = fingerprint(&with_disk(image), &disk(USB_DISK)).expect("image fingerprint");
    assert_eq!(captured.drive_serial, "/home/user/stick.img");
}

#[test]
fn losses_list_every_destroyed_volume() {
    let inventory = desktop_with_usb();
    let usb_losses = losses(&inventory, &disk(USB_DISK));
    let names: Vec<_> = usb_losses.iter().map(ToString::to_string).collect();
    assert_eq!(names, ["Photos", "Secure"]);
    assert_eq!(
        usb_losses[1].filesystem,
        Some(crate::model::FilesystemKind::Ext4)
    );
    assert_eq!(usb_losses[1].used, Some(ByteSize(5_000_000_000)));

    let desktop_losses = losses(&inventory, &disk(NVME_DISK));
    assert_eq!(desktop_losses.len(), 4);
    assert_eq!(
        desktop_losses[3].filesystem,
        Some(crate::model::FilesystemKind::Swap)
    );

    let windows = losses(&inventory, &block(WINDOWS_BLOCK));
    assert_eq!(windows.len(), 1);
    assert_eq!(windows[0].name, "Windows");
    assert_eq!(losses(&inventory, &block(USB_CLEARTEXT))[0].name, "Secure");
}

#[test]
fn locked_and_empty_partitions_in_losses() {
    let inventory = single_partition(encrypted(USB_CLEARTEXT, None), gib(8), gib(0));
    let locked = losses(&inventory, &disk(USB_DISK));
    assert_eq!(
        locked,
        [Loss {
            name: "/dev/sdb2".to_owned(),
            filesystem: None,
            used: None
        }]
    );
    let inventory = single_partition(Content::Empty, gib(8), gib(0));
    assert!(losses(&inventory, &disk(USB_DISK)).is_empty());
}
