const PACKAGES: [(&str, &str); 9] = [
    ("mkfs.ext4", "e2fsprogs"),
    ("mkfs.vfat", "dosfstools"),
    ("mkfs.fat", "dosfstools"),
    ("mkfs.exfat", "exfatprogs"),
    ("mkfs.ntfs", "ntfs-3g"),
    ("mkfs.btrfs", "btrfs-progs"),
    ("mkfs.xfs", "xfsprogs"),
    ("mkfs.f2fs", "f2fs-tools"),
    ("mkswap", "util-linux"),
];

pub(crate) fn package_providing(tool: &str) -> &str {
    PACKAGES
        .iter()
        .find_map(|(binary, package)| (*binary == tool).then_some(*package))
        .unwrap_or(tool)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tools_map_to_the_package_that_installs_them() {
        assert_eq!(package_providing("mkfs.exfat"), "exfatprogs");
        assert_eq!(package_providing("mkfs.ntfs"), "ntfs-3g");
        assert_eq!(package_providing("unknown-tool"), "unknown-tool");
    }
}
