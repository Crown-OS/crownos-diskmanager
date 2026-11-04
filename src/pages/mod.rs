mod disk;
mod frame;
mod overview;

use crownui::prelude::{Styled, View, keyed};

use crate::app::ContextMenu;
use crate::state::Store;
use crate::util::selection::selected_disk;

pub fn content(store: Store, menu: ContextMenu) -> impl View {
    keyed(
        move |runtime| {
            store
                .selection
                .with(runtime, |selection| selected_disk(selection).cloned())
        },
        move |cx, disk| match disk {
            Some(disk) => disk::disk_page(cx, store, menu, disk).boxed(),
            None => overview::overview(cx, store).boxed(),
        },
    )
    .flex_1()
    .flex_col()
    .min_h(0.0)
}
