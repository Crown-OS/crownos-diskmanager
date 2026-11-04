use super::WHOLE_DISK_PARTITION_NUMBER;
use crate::model::{Disk, Partition};

impl Partition {
    /// Whether this region is a filesystem (or other content) written directly
    /// to a disk with no partition table; its `block` is the disk's own block,
    /// so it can't be deleted or resized as a partition.
    pub fn is_whole_disk(&self) -> bool {
        self.number == WHOLE_DISK_PARTITION_NUMBER
    }
}

impl Disk {
    /// The pseudo-partition standing for an unpartitioned disk's content, if any.
    pub fn whole_disk_content(&self) -> Option<&Partition> {
        self.partitions()
            .find(|partition| partition.is_whole_disk())
    }
}
