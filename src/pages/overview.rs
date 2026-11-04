use std::sync::Arc;

use crownos_disks::model::{ByteSize, Disk, Inventory};
use crownui::kit::{CalloutOptions, Tone, callout, card};
use crownui::prelude::{
    AnyView, Cx, Listeners, Role, Semantic, Styled, View, hstack, keyed, spacer, vstack,
};

use super::frame::page;
use crate::components::{
    UsageShare, caption, disk_icon, headline, health_pill, title, usage_meter,
};
use crate::state::{Selection, Store};
use crate::util::nothing::nothing;

pub const OVERVIEW_TITLE: &str = "Overview";
const CARD_PADDING: f32 = 18.0;
const CARD_GAP: f32 = 14.0;
const CARD_HEADER_GAP: f32 = 12.0;
const ICON_SIZE: f32 = 28.0;
const TEXT_GAP: f32 = 2.0;
const SECTION_GAP: f32 = 12.0;

pub fn overview(_cx: &mut Cx, store: Store) -> impl View + use<> {
    page(
        OVERVIEW_TITLE,
        keyed(
            move |runtime| store.inventory.get(runtime),
            move |_, inventory: Arc<Inventory>| {
                vstack((
                    title(OVERVIEW_TITLE),
                    totals(&inventory),
                    empty_hint(&inventory),
                    disk_cards(store, &inventory),
                ))
                .gap(SECTION_GAP)
            },
        )
        .flex_col(),
    )
}

fn totals(inventory: &Inventory) -> impl View + use<> {
    let (used, total) = inventory
        .disks
        .iter()
        .flat_map(Disk::partitions)
        .filter_map(|partition| partition.content.volume())
        .filter(|volume| volume.is_mounted())
        .filter_map(|volume| volume.usage)
        .fold((0, 0), |(used, total), usage| {
            (used + usage.used.bytes(), total + usage.total.bytes())
        });
    let free = ByteSize(total).saturating_sub(ByteSize(used));
    caption(format!(
        "{} used · {free} free across mounted volumes",
        ByteSize(used)
    ))
}

fn empty_hint(inventory: &Inventory) -> AnyView {
    match inventory.disks.len() {
        0 => callout(Tone::Neutral, "No disks found")
            .body("Connect a drive or open a disk image.")
            .boxed(),
        1 => callout(Tone::Neutral, "Only the system disk is connected")
            .body("Drives you plug in and disk images you open appear here.")
            .boxed(),
        _ => nothing().boxed(),
    }
}

fn disk_cards(store: Store, inventory: &Inventory) -> impl View + use<> {
    let cards: Vec<AnyView> = inventory
        .disks
        .iter()
        .map(|disk| disk_card(store, disk).boxed())
        .collect();
    vstack(cards).gap(SECTION_GAP)
}

fn disk_card(store: Store, disk: &Disk) -> impl View + use<> {
    let id = disk.id.clone();
    let shares: Vec<UsageShare> = disk
        .partitions()
        .enumerate()
        .filter_map(|(color, partition)| {
            partition
                .content
                .volume()
                .and_then(|volume| volume.usage)
                .map(|usage| UsageShare {
                    used: usage.used,
                    color,
                })
        })
        .collect();
    let header = hstack((
        disk_icon(disk.kind, ICON_SIZE),
        vstack((
            headline(disk.name.clone()),
            caption(format!("{} · {}", disk.size, disk.kind.label())),
        ))
        .gap(TEXT_GAP),
        spacer(),
        health_pill(&disk.health),
    ))
    .gap(CARD_HEADER_GAP)
    .items_center();
    card((
        header,
        usage_meter(format!("{} usage", disk.name), shares, disk.size),
    ))
    .p(CARD_PADDING)
    .gap(CARD_GAP)
    .self_stretch()
    .role(Role::Button)
    .label(disk.name.clone())
    .on_click(move |cx, _| store.selection.set(cx, Selection::Disk(id.clone())))
}
