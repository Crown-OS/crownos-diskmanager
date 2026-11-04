use std::path::Path;

use crate::model::{ByteSize, Usage};

/// Space of the filesystem mounted at `mount_point`, via unprivileged `statvfs`.
///
/// `used` counts allocated blocks; `total` is `used` plus what an
/// unprivileged user can still write, so `Usage::available` excludes blocks
/// reserved for root (5 % on ext4 by default) and matches `df`'s "Avail".
pub(crate) fn filesystem_usage(mount_point: &Path) -> Option<Usage> {
    let stats = rustix::fs::statvfs(mount_point).ok()?;
    let used_blocks = stats.f_blocks.saturating_sub(stats.f_bfree);
    let block_size = stats.f_frsize;
    Some(Usage {
        total: ByteSize(
            used_blocks
                .saturating_add(stats.f_bavail)
                .saturating_mul(block_size),
        ),
        used: ByteSize(used_blocks.saturating_mul(block_size)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_filesystem_reports_usage() {
        let usage = filesystem_usage(Path::new("/")).unwrap();
        assert!(usage.total >= usage.used);
        assert!(usage.total.bytes() > 0);
    }

    #[test]
    fn missing_mount_point_has_no_usage() {
        assert_eq!(
            filesystem_usage(Path::new("/nonexistent/crownos-disks")),
            None
        );
    }
}
