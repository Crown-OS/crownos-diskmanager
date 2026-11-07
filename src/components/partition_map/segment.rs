use crownui::kit::{TEXT_SIZE, use_theme};
use crownui::prelude::{
    Cx, Listeners, Role, Runtime, Semantic, Signal, Styled, TRANSPARENT, View, bind, component,
    hstack, text, vstack, zstack,
};

use super::model::{MapSegment, SegmentPaint};
use crate::components::disk_icon::lock_icon;
use crate::components::text::strong_caption;
use crate::state::Selection;
use crate::util::partition_colors::{partition_fill, partition_usage_color};
use crate::util::tint::tint;

const SEGMENT_RADIUS: f32 = 8.0;
const SEGMENT_MIN_WIDTH: f32 = 10.0;
const SEGMENT_PADDING: f32 = 10.0;
const SELECTION_RING: f32 = 2.0;
const FREE_BORDER: f32 = 1.0;
const LABEL_GAP: f32 = 2.0;
const LOCK_SIZE: f32 = 12.0;
const USAGE_STRIP_HEIGHT: f32 = 4.0;

pub(super) fn segment<F>(segment: MapSegment, selection: Signal<Selection>, on_menu: F) -> impl View
where
    F: Fn(&mut Cx, Selection) + Copy + 'static,
{
    component(move |cx| {
        let theme = use_theme(cx);
        let MapSegment {
            region,
            title,
            detail,
            description,
            weight,
            used,
            paint,
            labelled,
        } = segment;
        let is_selected = {
            let region = region.clone();
            move |runtime: &mut Runtime| selection.with(runtime, |current| *current == region)
        };
        let border = match paint {
            SegmentPaint::Partition { .. } => 0.0,
            SegmentPaint::Free => FREE_BORDER,
        };
        let base = tint(theme, move |theme| match paint {
            SegmentPaint::Partition { color, .. } => partition_fill(theme, color),
            SegmentPaint::Free => theme.surface.sunken,
        });
        let usage_strip = match (paint, used) {
            (SegmentPaint::Partition { color, .. }, Some(fraction)) => Some(
                vstack(())
                    .absolute()
                    .bottom(0.0)
                    .left(0.0)
                    .h(USAGE_STRIP_HEIGHT)
                    .w_frac(fraction.clamp(0.0, 1.0))
                    .bg(tint(theme, move |theme| {
                        partition_usage_color(theme, color)
                    })),
            ),
            _ => None,
        };
        let lock = matches!(paint, SegmentPaint::Partition { locked: true, .. })
            .then(|| lock_icon(LOCK_SIZE));
        let label = labelled.then(|| {
            vstack((
                hstack((
                    lock,
                    text(title)
                        .text_size(TEXT_SIZE)
                        .font_medium()
                        .truncate()
                        .text_color(tint(theme, |theme| theme.text.primary)),
                ))
                .gap(LABEL_GAP * 2.0)
                .items_center(),
                strong_caption(detail),
            ))
            .gap(LABEL_GAP)
            .p(SEGMENT_PADDING)
            .min_w(0.0)
            .pointer_events_none()
        });
        let ring_selected = is_selected.clone();
        let click_region = region.clone();
        zstack((usage_strip, label))
            .grow(weight)
            .shrink(1.0)
            .basis(0.0)
            .min_w(SEGMENT_MIN_WIDTH)
            .h_full()
            .rounded(SEGMENT_RADIUS)
            .overflow_hidden()
            .bg(base)
            .border(border)
            .border_color(tint(theme, |theme| theme.surface.border))
            .ring(
                SELECTION_RING,
                bind(move |runtime| {
                    let chosen = ring_selected(runtime);
                    theme.with(runtime, |theme| {
                        if chosen {
                            theme.accent.end
                        } else {
                            TRANSPARENT
                        }
                    })
                }),
            )
            .role(Role::Button)
            .label(description)
            .focusable(true)
            .on_click(move |cx, _| selection.set(cx, click_region.clone()))
            .on_context_menu(move |cx, event| {
                event.stop_propagation();
                selection.set(cx, region.clone());
                on_menu(cx, region.clone());
            })
    })
}
