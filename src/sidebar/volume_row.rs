use crownos_disks::model::DiskId;
use crownui::kit::{SidebarItemOptions, sidebar_subitem, use_theme};
use crownui::prelude::{
    Cx, Listeners, Role, Runtime, Semantic, Signal, Styled, TRANSPARENT, View, bind, vstack,
};

use super::entry::{SidebarVolume, VolumeState};
use crate::app::ContextMenu;
use crate::state::{Selection, Store};

const DOT_SIZE: f32 = 7.0;
const DOT_BORDER: f32 = 1.5;

pub fn volume_row(
    cx: &mut Cx,
    store: Store,
    menu: ContextMenu,
    disk: DiskId,
    volume: Signal<SidebarVolume>,
) -> impl View + use<> {
    let block = volume.with_untracked(cx, |volume| volume.block.clone());
    let region = Selection::Partition { disk, block };
    let selected = region.clone();
    let clicked = region.clone();
    sidebar_subitem(
        bind(move |runtime| volume.with(runtime, |volume| volume.title.clone())),
        bind(move |runtime| {
            store
                .selection
                .with(runtime, |selection| *selection == selected)
        }),
    )
    .trailing_text(bind(move |runtime| {
        volume.with(runtime, |volume| volume.content.clone())
    }))
    .trailing(state_dot(cx, volume))
    .on_click(move |cx, _| store.selection.set(cx, clicked.clone()))
    .on_context_menu(move |cx, event| {
        event.stop_propagation();
        menu.open(cx, region.clone());
    })
}

fn state_dot(cx: &mut Cx, volume: Signal<SidebarVolume>) -> impl View + use<> {
    let theme = use_theme(cx);
    let state = move |runtime: &mut Runtime| volume.with(runtime, |volume| volume.state);
    vstack(())
        .size(DOT_SIZE)
        .rounded(DOT_SIZE / 2.0)
        .border(DOT_BORDER)
        .shrink(0.0)
        .bg(bind(move |runtime| match state(runtime) {
            VolumeState::Mounted => theme.with(runtime, |theme| theme.status.success),
            VolumeState::Unmounted | VolumeState::Locked => TRANSPARENT,
        }))
        .border_color(bind(move |runtime| {
            let current = state(runtime);
            theme.with(runtime, |theme| match current {
                VolumeState::Mounted => theme.status.success,
                VolumeState::Unmounted => theme.text.muted,
                VolumeState::Locked => theme.status.warning,
            })
        }))
        .role(Role::Image)
        .label(bind(move |runtime| match state(runtime) {
            VolumeState::Mounted => "Mounted".to_owned(),
            VolumeState::Unmounted => "Not mounted".to_owned(),
            VolumeState::Locked => "Locked".to_owned(),
        }))
}
