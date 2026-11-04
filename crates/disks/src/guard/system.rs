use std::path::Path;

use crate::model::{Content, Disk};

const SYSTEM_MOUNT_POINTS: [&str; 7] = ["/", "/boot", "/efi", "/boot/efi", "/home", "/usr", "/var"];
const LIVE_MEDIUM_MOUNT_POINT: &str = "/run/archiso/bootmnt";

/// How much the running system depends on a disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DiskRole {
    Ordinary,
    System,
    LiveMedium,
}

pub(crate) fn disk_role(disk: &Disk) -> DiskRole {
    let contents = || disk.partitions().map(|partition| &partition.content);
    if contents()
        .any(|content| mounted_at(content, &|path| path == Path::new(LIVE_MEDIUM_MOUNT_POINT)))
    {
        DiskRole::LiveMedium
    } else if contents().any(in_system_use) {
        DiskRole::System
    } else {
        DiskRole::Ordinary
    }
}

/// True when the content (or the cleartext inside it) backs a system mount point or active swap.
pub(crate) fn in_system_use(content: &Content) -> bool {
    match content {
        Content::Swap { active } => *active,
        Content::Encrypted(encryption) => encryption
            .unlocked
            .as_ref()
            .is_some_and(|unlocked| in_system_use(&unlocked.content)),
        _ => mounted_at(content, &is_system_mount_point),
    }
}

fn is_system_mount_point(path: &Path) -> bool {
    SYSTEM_MOUNT_POINTS
        .iter()
        .any(|system| path == Path::new(system))
}

fn mounted_at(content: &Content, matches: &dyn Fn(&Path) -> bool) -> bool {
    content
        .volume()
        .is_some_and(|volume| volume.mount_points.iter().any(|path| matches(path)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guard::test_inventory;
    use crate::model::{Encryption, LuksVersion, Unlocked};

    #[test]
    fn classifies_disks() {
        let inventory = test_inventory::desktop_with_usb();
        assert_eq!(disk_role(&inventory.disks[0]), DiskRole::System);
        assert_eq!(disk_role(&inventory.disks[1]), DiskRole::Ordinary);
        assert_eq!(
            disk_role(&test_inventory::live_medium()),
            DiskRole::LiveMedium
        );
    }

    #[test]
    fn every_system_mount_point_counts() {
        SYSTEM_MOUNT_POINTS.iter().for_each(|mount_point| {
            assert!(
                in_system_use(&test_inventory::filesystem_mounted_at(mount_point)),
                "{mount_point}"
            );
        });
        assert!(!in_system_use(&test_inventory::filesystem_mounted_at(
            "/run/media/user/data"
        )));
        assert!(!in_system_use(&test_inventory::filesystem_mounted_at(
            "/homework"
        )));
    }

    #[test]
    fn active_swap_and_encrypted_root_count() {
        assert!(in_system_use(&Content::Swap { active: true }));
        assert!(!in_system_use(&Content::Swap { active: false }));
        let encrypted_root = Content::Encrypted(Encryption {
            version: LuksVersion::Luks2,
            header_size: crate::model::ByteSize(16 << 20),
            unlocked: Some(Box::new(Unlocked {
                cleartext: crate::model::BlockId::new("/dm_0"),
                content: test_inventory::filesystem_mounted_at("/"),
            })),
        });
        assert!(in_system_use(&encrypted_root));
    }
}
