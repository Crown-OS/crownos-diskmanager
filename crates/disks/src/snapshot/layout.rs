use std::ops::Range;

use super::content;
use super::object_view::ObjectView;
use crate::model::{BlockId, ByteSize, Content, Partition, Region, TableKind};
use crate::object_map::{ObjectMap, interface};

/// Partitioning tools (libfdisk, used by UDisks) start the first partition at 1 MiB.
pub(super) const FIRST_USABLE_OFFSET: u64 = ByteSize::MIB.bytes();
/// Space kept free at the end of a GPT disk for the backup header and entries
/// (33 sectors), rounded up to the 1 MiB alignment UDisks uses.
pub(super) const GPT_BACKUP_RESERVE: u64 = ByteSize::MIB.bytes();
/// Gaps smaller than this are alignment slack, not usable free space.
pub(super) const MIN_FREE_REGION: u64 = ByteSize::MIB.bytes();
const ALIGNMENT: u64 = ByteSize::MIB.bytes();

/// Partition number of the pseudo-partition that stands for a filesystem (or
/// LUKS volume, etc.) written directly to a disk without a partition table.
pub const WHOLE_DISK_PARTITION_NUMBER: u32 = 0;

pub(super) struct Layout {
    pub table: Option<TableKind>,
    pub regions: Vec<Region>,
}

/// The disk's partitions and free space in offset order.
///
/// - With a partition table: every partition plus free gaps of at least 1 MiB.
/// - Without a table but with content (a "superfloppy" USB stick, an ISO):
///   one pseudo-partition numbered [`WHOLE_DISK_PARTITION_NUMBER`] spanning the
///   disk, whose `block` is the disk's own block.
/// - Blank: one free region spanning the disk.
pub(super) fn layout(map: &ObjectMap, disk_block: ObjectView<'_>) -> Layout {
    let disk_size = disk_block.uint(interface::BLOCK, "Size");
    if !disk_block.has(interface::PARTITION_TABLE) {
        return unpartitioned(map, disk_block, disk_size);
    }
    let table = match disk_block.text(interface::PARTITION_TABLE, "Type") {
        "gpt" => Some(TableKind::Gpt),
        "dos" => Some(TableKind::Mbr),
        _ => None,
    };
    let mut partitions: Vec<_> = ObjectView::all(map)
        .filter(|block| block.object(interface::PARTITION, "Table") == Some(disk_block.path))
        .collect();
    partitions.sort_by_key(|block| block.uint(interface::PARTITION, "Offset"));

    let occupied = partitions.iter().map(|block| extent(*block));
    let free = free_regions(occupied, usable_range(table, disk_size));
    let mut regions: Vec<Region> = partitions
        .iter()
        .filter(|block| !block.flag(interface::PARTITION, "IsContainer"))
        .map(|block| Region::Partition(partition(map, *block)))
        .chain(free)
        .collect();
    regions.sort_by_key(Region::offset);
    Layout { table, regions }
}

fn unpartitioned(map: &ObjectMap, disk_block: ObjectView<'_>, disk_size: u64) -> Layout {
    let content = content::resolve(map, disk_block);
    let region = match content {
        Content::Empty => Region::Free {
            offset: ByteSize(0),
            size: ByteSize(disk_size),
        },
        content => Region::Partition(Partition {
            block: BlockId::new(disk_block.path),
            device: device(disk_block),
            number: WHOLE_DISK_PARTITION_NUMBER,
            offset: ByteSize(0),
            size: ByteSize(disk_size),
            name: String::new(),
            type_id: String::new(),
            uuid: String::new(),
            content,
        }),
    };
    Layout {
        table: None,
        regions: vec![region],
    }
}

pub(super) fn device(block: ObjectView<'_>) -> String {
    let preferred = block.text(interface::BLOCK, "PreferredDevice");
    match preferred.is_empty() {
        true => block.text(interface::BLOCK, "Device").to_owned(),
        false => preferred.to_owned(),
    }
}

fn partition(map: &ObjectMap, block: ObjectView<'_>) -> Partition {
    Partition {
        block: BlockId::new(block.path),
        device: device(block),
        number: u32::try_from(block.uint(interface::PARTITION, "Number")).unwrap_or_default(),
        offset: ByteSize(block.uint(interface::PARTITION, "Offset")),
        size: ByteSize(block.uint(interface::PARTITION, "Size")),
        name: block.text(interface::PARTITION, "Name").to_owned(),
        type_id: block.text(interface::PARTITION, "Type").to_owned(),
        uuid: block.text(interface::PARTITION, "UUID").to_owned(),
        content: content::resolve(map, block),
    }
}

fn extent(block: ObjectView<'_>) -> Range<u64> {
    let offset = block.uint(interface::PARTITION, "Offset");
    offset..offset.saturating_add(block.uint(interface::PARTITION, "Size"))
}

pub(super) fn usable_range(table: Option<TableKind>, disk_size: u64) -> Range<u64> {
    let end = match table {
        Some(TableKind::Gpt) => disk_size.saturating_sub(GPT_BACKUP_RESERVE),
        _ => disk_size,
    };
    FIRST_USABLE_OFFSET.min(end)..end
}

/// Gaps between `occupied` extents (sorted by start, possibly overlapping, as
/// MBR logical partitions overlap their extended container), aligned inwards
/// to 1 MiB and kept when at least [`MIN_FREE_REGION`].
pub(super) fn free_regions(
    occupied: impl Iterator<Item = Range<u64>>,
    usable: Range<u64>,
) -> Vec<Region> {
    let mut regions = Vec::new();
    let mut cursor = usable.start;
    let mut push_gap = |start: u64, end: u64| {
        let aligned_start = start.next_multiple_of(ALIGNMENT);
        let aligned_end = end.min(usable.end) / ALIGNMENT * ALIGNMENT;
        if aligned_end.saturating_sub(aligned_start) >= MIN_FREE_REGION {
            regions.push(Region::Free {
                offset: ByteSize(aligned_start),
                size: ByteSize(aligned_end - aligned_start),
            });
        }
    };
    for extent in occupied {
        if extent.start > cursor {
            push_gap(cursor, extent.start);
        }
        cursor = cursor.max(extent.end);
    }
    push_gap(cursor, usable.end);
    regions
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIB: u64 = ByteSize::MIB.bytes();
    const GIB: u64 = ByteSize::GIB.bytes();

    fn free(offset: u64, size: u64) -> Region {
        Region::Free {
            offset: ByteSize(offset),
            size: ByteSize(size),
        }
    }

    #[test]
    fn gpt_reserves_the_backup_table() {
        assert_eq!(
            usable_range(Some(TableKind::Gpt), 10 * GIB),
            MIB..10 * GIB - MIB
        );
        assert_eq!(usable_range(Some(TableKind::Mbr), 10 * GIB), MIB..10 * GIB);
    }

    #[test]
    fn empty_table_is_one_free_region() {
        assert_eq!(
            free_regions([].into_iter(), MIB..10 * GIB - MIB),
            [free(MIB, 10 * GIB - 2 * MIB)]
        );
    }

    #[test]
    fn gaps_between_and_after_partitions_are_free() {
        let occupied = [MIB..GIB, 2 * GIB..3 * GIB];
        assert_eq!(
            free_regions(occupied.into_iter(), MIB..4 * GIB),
            [free(GIB, GIB), free(3 * GIB, GIB)]
        );
    }

    #[test]
    fn slack_under_one_mebibyte_is_ignored() {
        let occupied = [MIB..GIB, GIB + 4096..2 * GIB - 512 * 1024];
        assert_eq!(free_regions(occupied.into_iter(), MIB..2 * GIB), []);
    }

    #[test]
    fn unaligned_gaps_shrink_to_whole_mebibytes() {
        let occupied = [MIB..GIB + 4096, GIB + 3 * MIB..2 * GIB];
        assert_eq!(
            free_regions(occupied.into_iter(), MIB..2 * GIB),
            [free(GIB + MIB, 2 * MIB)]
        );
    }

    #[test]
    fn logical_partitions_inside_a_container_are_not_free_space() {
        let occupied = [MIB..GIB, GIB..4 * GIB, GIB + MIB..2 * GIB];
        assert_eq!(free_regions(occupied.into_iter(), MIB..4 * GIB), []);
    }
}
