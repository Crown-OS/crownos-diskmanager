mod model;
mod segment;

use crownui::ext::{Key, NamedKey};
use crownui::prelude::{Cx, Listeners, Role, Semantic, Signal, Styled, View, hstack};

pub use model::{MapSegment, map_segments};
use segment::segment;

use crate::state::Selection;

const MAP_HEIGHT: f32 = 64.0;
const SEGMENT_GAP: f32 = 4.0;
const RING_ROOM: f32 = 3.0;

pub fn partition_map<F>(
    segments: Vec<MapSegment>,
    selection: Signal<Selection>,
    on_menu: F,
) -> impl View
where
    F: Fn(&mut Cx, Selection) + Copy + 'static,
{
    let regions: Vec<Selection> = segments
        .iter()
        .map(|segment| segment.region.clone())
        .collect();
    let views: Vec<_> = segments
        .into_iter()
        .map(|entry| segment(entry, selection, on_menu))
        .collect();
    hstack(views)
        .gap(SEGMENT_GAP)
        .h(MAP_HEIGHT)
        .p(RING_ROOM)
        .w_full()
        .role(Role::Group)
        .label("Partition map")
        .on_key_down(move |cx, event| {
            let step = match event.key {
                Key::Named(NamedKey::ArrowLeft) => -1,
                Key::Named(NamedKey::ArrowRight) => 1,
                _ => return,
            };
            let current = selection.with(cx, |current| {
                regions.iter().position(|region| region == current)
            });
            if let Some(next) = neighbour(current, step, regions.len()) {
                selection.set(cx, regions[next].clone());
                event.prevent_default();
                event.stop_propagation();
            }
        })
}

fn neighbour(current: Option<usize>, step: isize, count: usize) -> Option<usize> {
    let last = count.checked_sub(1)?;
    Some(match current {
        None if step < 0 => last,
        None => 0,
        Some(index) => index.saturating_add_signed(step).min(last),
    })
}

#[cfg(test)]
mod tests {
    use super::neighbour;

    #[test]
    fn arrows_walk_the_map_and_stop_at_the_ends() {
        assert_eq!(neighbour(Some(0), -1, 3), Some(0));
        assert_eq!(neighbour(Some(1), 1, 3), Some(2));
        assert_eq!(neighbour(Some(2), 1, 3), Some(2));
        assert_eq!(neighbour(None, 1, 3), Some(0));
        assert_eq!(neighbour(None, -1, 3), Some(2));
        assert_eq!(neighbour(None, 1, 0), None);
    }
}
