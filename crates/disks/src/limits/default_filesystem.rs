use crate::model::{ByteSize, Disk, FilesystemKind, Removal};

const EXFAT_THRESHOLD: ByteSize = ByteSize(32_000_000_000);

pub(super) fn default_filesystem(disk: &Disk) -> FilesystemKind {
    let portable = matches!(disk.removal, Removal::Removable { .. }) || disk.kind.is_external();
    match (portable, disk.size >= EXFAT_THRESHOLD) {
        (true, true) => FilesystemKind::Exfat,
        (true, false) => FilesystemKind::Fat32,
        (false, _) => FilesystemKind::Ext4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guard::test_inventory;

    #[test]
    fn portable_disks_get_portable_filesystems() {
        let mut usb = test_inventory::usb_disk();
        assert_eq!(default_filesystem(&usb), FilesystemKind::Exfat);
        usb.size = ByteSize(EXFAT_THRESHOLD.0 - 1);
        assert_eq!(default_filesystem(&usb), FilesystemKind::Fat32);
        usb.size = EXFAT_THRESHOLD;
        assert_eq!(default_filesystem(&usb), FilesystemKind::Exfat);
        assert_eq!(
            default_filesystem(&test_inventory::desktop_disk()),
            FilesystemKind::Ext4
        );
    }
}
