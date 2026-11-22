use crownos_disks::model::{Disk, FilesystemKind, Partition, Volume};

use crate::util::fstab::FstabSources;

/// The data filesystem on `partition` that should be mounted for the user
/// without being asked: not firmware, swap, a disk image or an fstab entry.
pub fn automountable_volume<'disk>(
    disk: &Disk,
    partition: &'disk Partition,
    fstab: &FstabSources,
) -> Option<&'disk Volume> {
    let volume = partition.content.volume()?;
    let holds_data = !matches!(
        volume.filesystem,
        FilesystemKind::Swap | FilesystemKind::Other(_)
    );
    let eligible = holds_data
        && !volume.hidden
        && disk.backing_file.is_none()
        && !fstab.lists(partition, volume);
    eligible.then_some(volume)
}
