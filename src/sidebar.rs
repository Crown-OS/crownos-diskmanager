mod disk_row;
mod entry;
mod volume_row;

use crownui::kit::{icons, sidebar_item, sidebar_section};
use crownui::prelude::{
    AnyView, Cx, Listeners, Styled, View, ViewSeq, bind, for_each, show, spacer,
};

use crate::app::ContextMenu;
use crate::state::{DialogRequest, Selection, Store};
use crate::util::derived::derived;
use crate::util::nothing::nothing;
use disk_row::disk_row;
use entry::{Category, SidebarDisk, sidebar_disks};

pub const OVERVIEW_LABEL: &str = "Overview";
pub const OPEN_IMAGE_LABEL: &str = "Open Image…";

pub fn sidebar_menus(cx: &mut Cx, store: Store, menu: ContextMenu) -> impl ViewSeq + use<> {
    let overview = sidebar_item(
        OVERVIEW_LABEL,
        icons::LAYOUT_GRID,
        bind(move |runtime| {
            store
                .selection
                .with(runtime, |selection| *selection == Selection::Overview)
        }),
    )
    .on_click(move |cx, _| store.selection.set(cx, Selection::Overview));
    let sections: Vec<AnyView> = Category::ALL
        .into_iter()
        .map(|category| disk_section(cx, store, menu, category).boxed())
        .collect();
    let open_image = sidebar_item(OPEN_IMAGE_LABEL, icons::PLUS, false)
        .on_click(move |cx, _| store.open_dialog(cx, DialogRequest::OpenImage));
    (overview, sections, spacer(), open_image)
}

fn disk_section(
    cx: &mut Cx,
    store: Store,
    menu: ContextMenu,
    category: Category,
) -> impl View + use<> {
    let disks = derived(cx, move |runtime| {
        store
            .inventory
            .with(runtime, |inventory| sidebar_disks(inventory, category))
    });
    show(
        bind(move |runtime| disks.with(runtime, |disks| !disks.is_empty())),
        move || {
            sidebar_section(
                category.label(),
                for_each(
                    disks,
                    |disk: &SidebarDisk| disk.id.clone(),
                    move |cx, disk| disk_row(cx, store, menu, disk),
                ),
            )
        },
        nothing,
    )
    .flex_col()
}
