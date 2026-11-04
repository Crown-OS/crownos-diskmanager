use crownos_disks::model::{Disk, Region};

use crate::state::Selection;
use crate::util::region_text::{
    content_summary, free_description, free_title, is_locked, partition_description,
    partition_title, used_fraction,
};
use crate::util::selection::region_selection;

const MIN_WEIGHT_FRACTION: f64 = 0.02;
const LABEL_MIN_FRACTION: f64 = 0.08;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentPaint {
    Partition { color: usize, locked: bool },
    Free,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MapSegment {
    pub region: Selection,
    pub title: String,
    pub detail: String,
    pub description: String,
    pub weight: f32,
    pub used: Option<f32>,
    pub paint: SegmentPaint,
    pub labelled: bool,
}

pub fn map_segments(disk: &Disk) -> Vec<MapSegment> {
    let floor = disk.size.bytes() as f64 * MIN_WEIGHT_FRACTION;
    let weight_of = |region: &Region| (region.size().bytes() as f64).max(floor);
    let total: f64 = disk.layout.iter().map(weight_of).sum();
    let mut partitions_seen = 0;
    disk.layout
        .iter()
        .map(|region| {
            let weight = weight_of(region);
            let labelled = total > 0.0 && weight / total >= LABEL_MIN_FRACTION;
            let region_key = region_selection(&disk.id, region);
            match region {
                Region::Partition(partition) => {
                    let color = partitions_seen;
                    partitions_seen += 1;
                    MapSegment {
                        region: region_key,
                        title: partition_title(partition).into_owned(),
                        detail: format!(
                            "{} · {}",
                            content_summary(&partition.content),
                            partition.size
                        ),
                        description: partition_description(partition),
                        weight: weight as f32,
                        used: used_fraction(partition),
                        paint: SegmentPaint::Partition {
                            color,
                            locked: is_locked(&partition.content),
                        },
                        labelled,
                    }
                }
                Region::Free { size, .. } => MapSegment {
                    region: region_key,
                    title: free_title(disk.table.is_some()).to_owned(),
                    detail: size.to_string(),
                    description: free_description(disk.table.is_some(), *size),
                    weight: weight as f32,
                    used: None,
                    paint: SegmentPaint::Free,
                    labelled,
                },
            }
        })
        .collect()
}
