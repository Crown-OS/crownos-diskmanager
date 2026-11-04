use crownos_disks::Command;
use crownos_disks::model::DiskId;
use crownui::kit::{
    ButtonOptions, ButtonVariant, SidebarItemOptions, icon_button, icons, sidebar_collapse,
    sidebar_column, sidebar_item,
};
use crownui::prelude::{
    AnyView, Cx, Listeners, Semantic, Signal, Styled, View, bind, for_each, hstack, keyed, vstack,
    zstack,
};

use super::entry::{Detach, SidebarDisk, SidebarVolume};
use super::volume_row::volume_row;
use crate::app::ContextMenu;
use crate::components::disk_glyph;
use crate::state::{Selection, Store};
use crate::util::derived::derived;
use crate::util::nothing::nothing;

const ROW_GAP: f32 = 4.0;
const TRAILING_GAP: f32 = 2.0;
const CONTROL_SIZE: f32 = 28.0;
const ROW_HEIGHT: f32 = 36.0;
const CONTROLS_INSET: f32 = 4.0;

pub fn disk_row(
    cx: &mut Cx,
    store: Store,
    menu: ContextMenu,
    disk: Signal<SidebarDisk>,
) -> impl View + use<> {
    let (id, kind, detach) =
        disk.with_untracked(cx, |disk| (disk.id.clone(), disk.kind, disk.detach));
    let open = cx.signal(true);
    let selected_id = id.clone();
    let click_id = id.clone();
    let menu_id = id.clone();
    let controls = hstack((detach_button(store, &id, detach), disclosure(open)))
        .gap(TRAILING_GAP)
        .items_center()
        .absolute()
        .top(0.0)
        .h(ROW_HEIGHT)
        .right(CONTROLS_INSET);
    let row = sidebar_item(
        bind(move |runtime| disk.with(runtime, |disk| disk.name.clone())),
        disk_glyph(kind),
        bind(move |runtime| {
            store.selection.with(runtime, |selection| {
                *selection == Selection::Disk(selected_id.clone())
            })
        }),
    )
    .label(bind(move |runtime| {
        disk.with(runtime, |disk| disk.name.clone())
    }))
    .trailing_text(bind(move |runtime| {
        disk.with(runtime, |disk| disk.size.to_string())
    }))
    .trailing(vstack(()).w(controls_width(detach)).shrink(0.0))
    .on_click(move |cx, _| {
        store.selection.set(cx, Selection::Disk(click_id.clone()));
        open.set(cx, true);
    })
    .on_context_menu(move |cx, event| {
        event.stop_propagation();
        menu.open(cx, Selection::Disk(menu_id.clone()));
    });
    let header = zstack((row, controls));
    let volumes = derived(cx, move |runtime| {
        disk.with(runtime, |disk| disk.volumes.clone())
    });
    let rows = for_each(
        volumes,
        |volume: &SidebarVolume| volume.block.clone(),
        move |cx, volume| volume_row(cx, store, menu, id.clone(), volume),
    );
    vstack((header, sidebar_collapse(sidebar_column(rows), open))).gap(ROW_GAP)
}

fn detach_button(store: Store, id: &DiskId, detach: Detach) -> AnyView {
    let (label, command) = match detach {
        Detach::Fixed => return nothing().boxed(),
        Detach::Eject => ("Eject", Command::Eject(id.clone())),
        Detach::CloseImage => ("Close Image", Command::CloseImage(id.clone())),
    };
    icon_button(icons::ARROW_UP_FROM_LINE, label)
        .variant(ButtonVariant::Ghost)
        .self_center()
        .on_click(move |cx, _| store.send(cx, command.clone()))
        .boxed()
}

fn controls_width(detach: Detach) -> f32 {
    match detach {
        Detach::Fixed => CONTROL_SIZE,
        Detach::Eject | Detach::CloseImage => CONTROL_SIZE * 2.0 + TRAILING_GAP,
    }
}

fn disclosure(open: Signal<bool>) -> impl View {
    keyed(
        move |runtime| open.get(runtime),
        move |_, is_open| {
            let (glyph, label) = match is_open {
                true => (icons::CHEVRON_DOWN, "Hide Volumes"),
                false => (icons::CHEVRON_RIGHT, "Show Volumes"),
            };
            icon_button(glyph, label)
                .variant(ButtonVariant::Ghost)
                .on_click(move |cx, _| open.update(cx, |open| *open = !*open))
        },
    )
}
