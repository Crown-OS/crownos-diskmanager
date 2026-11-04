use std::borrow::Cow;

use crownos_disks::model::{ByteSize, Content, Encryption, Partition};

const PERCENT: f32 = 100.0;
const WHOLE_DISK_NUMBER: u32 = 0;
const WHOLE_DISK_TITLE: &str = "Volume";

pub fn partition_title(partition: &Partition) -> Cow<'_, str> {
    let label = partition
        .content
        .volume()
        .map(|volume| volume.label.as_str())
        .unwrap_or_default();
    match (label, partition.name.as_str()) {
        (label, _) if !label.is_empty() => Cow::Borrowed(label),
        (_, name) if !name.is_empty() => Cow::Borrowed(name),
        _ if partition.number == WHOLE_DISK_NUMBER => Cow::Borrowed(WHOLE_DISK_TITLE),
        _ => Cow::Owned(format!("Partition {}", partition.number)),
    }
}

pub fn content_summary(content: &Content) -> Cow<'_, str> {
    match content {
        Content::Filesystem(volume) => Cow::Borrowed(volume.filesystem.short_name()),
        Content::Encrypted(Encryption { unlocked: None, .. }) => Cow::Borrowed("Encrypted"),
        Content::Encrypted(Encryption {
            unlocked: Some(unlocked),
            ..
        }) => Cow::Owned(format!("Encrypted {}", content_summary(&unlocked.content))),
        Content::Swap { .. } => Cow::Borrowed("Swap"),
        Content::Member { kind } => Cow::Borrowed(kind),
        Content::Unknown { id_type } if !id_type.is_empty() => Cow::Borrowed(id_type),
        Content::Unknown { .. } => Cow::Borrowed("Unknown"),
        Content::Empty => Cow::Borrowed("Empty"),
    }
}

pub fn is_locked(content: &Content) -> bool {
    matches!(
        content,
        Content::Encrypted(Encryption { unlocked: None, .. })
    )
}

pub fn used_fraction(partition: &Partition) -> Option<f32> {
    partition
        .content
        .volume()
        .and_then(|volume| volume.usage)
        .map(|usage| usage.fraction_used())
}

pub fn whole_percent(fraction: f32) -> u32 {
    (fraction.clamp(0.0, 1.0) * PERCENT).round() as u32
}

pub fn partition_description(partition: &Partition) -> String {
    let title = partition_title(partition);
    let content = content_summary(&partition.content);
    match used_fraction(partition) {
        Some(fraction) => format!(
            "{title}, {content}, {}, {} % used",
            partition.size,
            whole_percent(fraction)
        ),
        None => format!("{title}, {content}, {}", partition.size),
    }
}

pub fn free_title(has_table: bool) -> &'static str {
    if has_table {
        "Free space"
    } else {
        "Unallocated"
    }
}

pub fn free_description(has_table: bool, size: ByteSize) -> String {
    format!("{}, {size}", free_title(has_table))
}

#[cfg(test)]
mod tests {
    use crownos_disks::model::{BlockId, FilesystemKind, Usage, Volume};

    use super::*;

    fn partition(content: Content) -> Partition {
        Partition {
            block: BlockId::new("/block/nvme0n1p2"),
            device: "/dev/nvme0n1p2".into(),
            number: 2,
            offset: ByteSize::MIB,
            size: ByteSize(475_000_000_000),
            name: String::new(),
            type_id: String::new(),
            uuid: String::new(),
            content,
        }
    }

    fn ext4(label: &str, used: u64) -> Content {
        Content::Filesystem(Volume {
            filesystem: FilesystemKind::Ext4,
            label: label.into(),
            uuid: String::new(),
            mount_points: Vec::new(),
            usage: Some(Usage {
                total: ByteSize(475_000_000_000),
                used: ByteSize(used),
            }),
        })
    }

    #[test]
    fn a_segment_reads_name_format_size_and_usage() {
        let root = partition(ext4("Root", 294_500_000_000));
        assert_eq!(
            partition_description(&root),
            "Root, ext4, 475 GB, 62 % used"
        );
    }

    #[test]
    fn unnamed_partitions_fall_back_to_their_number() {
        let blank = partition(Content::Empty);
        assert_eq!(partition_title(&blank), "Partition 2");
        assert_eq!(partition_description(&blank), "Partition 2, Empty, 475 GB");
    }

    #[test]
    fn a_whole_disk_filesystem_is_a_volume_not_partition_zero() {
        let mut whole = partition(Content::Empty);
        whole.number = 0;
        assert_eq!(partition_title(&whole), "Volume");
    }

    #[test]
    fn locked_volumes_read_as_encrypted() {
        let locked = Content::Encrypted(Encryption {
            version: crownos_disks::model::LuksVersion::Luks2,
            header_size: ByteSize(16 << 20),
            unlocked: None,
        });
        assert!(is_locked(&locked));
        assert_eq!(content_summary(&locked), "Encrypted");
    }
}
