use crownos_disks::model::ByteSize;
use crownui::kit::{Segment, SegmentedBarOptions, segmented_bar, use_theme};
use crownui::prelude::{Role, Semantic, Styled, View, component, vstack};

use super::text::caption;
use crate::util::partition_colors::partition_color;
use crate::util::region_text::whole_percent;

const BAR_HEIGHT: f32 = 8.0;
const CAPTION_GAP: f32 = 6.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UsageShare {
    pub used: ByteSize,
    pub color: usize,
}

pub fn usage_meter(name: impl Into<String>, shares: Vec<UsageShare>, total: ByteSize) -> impl View {
    let name = name.into();
    let used = ByteSize(shares.iter().map(|share| share.used.bytes()).sum());
    let free = total.saturating_sub(used);
    let fraction = match total.bytes() {
        0 => 0.0,
        bytes => (used.bytes() as f64 / bytes as f64) as f32,
    };
    component(move |cx| {
        let theme = use_theme(cx);
        let bar = segmented_bar(Vec::new())
            .segments_from(move |runtime| {
                theme.with(runtime, |theme| {
                    shares
                        .iter()
                        .map(|share| {
                            Segment::new(
                                share.used.bytes() as f32,
                                partition_color(theme, share.color),
                            )
                        })
                        .chain([Segment::new(free.bytes() as f32, theme.control.track)])
                        .collect()
                })
            })
            .h(BAR_HEIGHT)
            .w_full();
        vstack((
            bar,
            caption(format!("{used} used of {total} · {free} free")),
        ))
        .gap(CAPTION_GAP)
        .w_full()
        .role(Role::ProgressBar)
        .label(name)
        .value(format!("{} %", whole_percent(fraction)))
    })
}
