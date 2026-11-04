mod details;
mod header;
mod health;
mod interrupted;
mod region_card;

use crownos_disks::Command;
use crownos_disks::model::{Disk, DiskId, DiskKind};
use crownui::prelude::{Cx, Memo, Runtime, Styled, View, keyed};

use super::frame::page;
use crate::app::ContextMenu;
use crate::components::{map_segments, partition_map};
use crate::state::Store;
use crate::util::nothing::nothing;

pub fn disk_page(cx: &mut Cx, store: Store, menu: ContextMenu, id: DiskId) -> impl View + use<> {
    let lookup = id.clone();
    let disk: Memo<Option<Disk>> = cx.memo(move |runtime| {
        store
            .inventory
            .with(runtime, |inventory| inventory.disk(&lookup).cloned())
    });
    let (name, reports_health) = cx.untrack(|runtime| {
        disk.with(runtime, |disk| match disk {
            Some(disk) => (disk.name.clone(), disk.kind != DiskKind::Image),
            None => (String::new(), false),
        })
    });
    if reports_health {
        store.send(cx, Command::RefreshHealth(id.clone()));
    }
    page(
        name,
        (
            section(disk, |disk| Some(disk.clone()), header::header),
            section(
                disk,
                |disk| Some(map_segments(disk)),
                move |_, segments| {
                    partition_map(segments, store.selection, move |cx, region| {
                        menu.open(cx, region)
                    })
                },
            ),
            region_card::region_card(store, disk),
            interrupted::interrupted(store, id),
            section(
                disk,
                |disk| (disk.kind != DiskKind::Image).then(|| disk.health.clone()),
                health::health,
            ),
            section(disk, |disk| Some(disk.clone()), details::details),
        ),
    )
}

fn section<T, V>(
    disk: Memo<Option<Disk>>,
    extract: fn(&Disk) -> Option<T>,
    mut build: impl FnMut(&mut Cx, T) -> V + 'static,
) -> impl View
where
    T: Clone + PartialEq + 'static,
    V: View,
{
    keyed(
        move |runtime: &mut Runtime| disk.with(runtime, |disk| disk.as_ref().and_then(extract)),
        move |cx, value| match value {
            Some(value) => build(cx, value).boxed(),
            None => nothing().boxed(),
        },
    )
    .flex_col()
}
