use std::path::PathBuf;
use std::sync::Arc;

use crownos_disks::Command;
use crownos_disks::command::{Compression, Target};
use crownos_disks::model::Partition;
use crownui::kit::{ButtonOptions, ButtonVariant, ToggleItem, button, toggle_group};
use crownui::prelude::{
    AnyView, Cx, Listeners, Semantic, Signal, Styled, View, bind, component, hstack, text, vstack,
};

use super::image_file::{ImageKind, display_name, pick_image, suggested_name};
use crate::dialogs::context::SheetContext;
use crate::dialogs::sheets::gone_sheet;
use crate::dialogs::sheets::layout::{
    actions, cancel_button, form, form_dialog, labeled, muted_text, sheet_title,
};
use crate::dialogs::target::Resolved;
use crate::util::portal::FileChoice;

const COMPRESSIONS: [(Compression, &str, &str); 3] = [
    (
        Compression::None,
        "None",
        "Same size as the source, fastest to create.",
    ),
    (
        Compression::Zstd,
        "Zstandard",
        "Recommended: much smaller and still fast.",
    ),
    (Compression::Xz, "XZ", "Smallest file, but slow to create."),
];
const GAP: f32 = 8.0;
const NO_FILE: &str = "No file chosen";

fn compression_at(index: usize) -> (Compression, &'static str) {
    COMPRESSIONS
        .get(index)
        .map_or((Compression::Zstd, ""), |(compression, _, caption)| {
            (*compression, caption)
        })
}

fn image_kind(resolved: Resolved<'_>) -> ImageKind {
    let partitions: Vec<&Partition> = match resolved {
        Resolved::Disk(disk) => disk.partitions().collect(),
        Resolved::Partition(_, partition) => vec![partition],
    };
    let disc = partitions
        .iter()
        .filter_map(|partition| partition.content.volume())
        .any(|volume| ImageKind::of_filesystem(volume.filesystem.udisks_type()) == ImageKind::Disc);
    match disc {
        true => ImageKind::Disc,
        false => ImageKind::Raw,
    }
}

fn compression_picker(selected: Signal<usize>) -> impl View {
    vstack((
        toggle_group(
            COMPRESSIONS
                .iter()
                .map(|(_, label, _)| ToggleItem::text(*label)),
            selected,
        )
        .label("Compression"),
        muted_text(bind(move |runtime| {
            compression_at(selected.get(runtime)).1.to_owned()
        })),
    ))
    .gap(GAP)
}

pub fn create_image_sheet(context: SheetContext, target: Target) -> AnyView {
    component(move |cx| {
        let inventory = context.store.inventory.with_untracked(cx, Arc::clone);
        let Some(resolved) = Resolved::find(&inventory, &target) else {
            return gone_sheet(context, "The disk".to_owned()).boxed();
        };
        let name = resolved.name().into_owned();
        let source = format!("{name} · {}", resolved.size());
        let kind = image_kind(resolved);
        let open = context.sheet_open(cx);
        let default_compression = match kind {
            ImageKind::Disc => Compression::None,
            ImageKind::Raw => Compression::Zstd,
        };
        let compression = cx.signal(
            COMPRESSIONS
                .iter()
                .position(|(compression, _, _)| *compression == default_compression)
                .unwrap_or_default(),
        );
        let destination: Signal<Option<PathBuf>> = cx.signal(None);
        let choosing = cx.signal(false);

        let choose = {
            let name = name.clone();
            move |cx: &mut Cx| {
                choosing.set(cx, true);
                let suggested_name =
                    suggested_name(&name, kind, compression_at(compression.get(cx)).0);
                pick_image(
                    cx,
                    context,
                    "Save Disk Image",
                    FileChoice::Save { suggested_name },
                    move |cx, path| {
                        choosing.set(cx, false);
                        if path.is_some() {
                            destination.set(cx, path);
                        }
                    },
                );
            }
        };
        let create = move |cx: &mut Cx| {
            let Some(destination) = destination.get(cx) else {
                return;
            };
            let compression = compression_at(compression.get(cx)).0;
            let command = Command::CreateImage {
                source: target.clone(),
                destination,
                compression,
            };
            context.launch(cx, target.clone(), command);
        };
        form_dialog(open, move || {
            let (choose, create) = (choose.clone(), create.clone());
            (
                sheet_title(format!("Create Image of “{name}”")),
                muted_text(source.clone()),
                form((
                    labeled("Compression", compression_picker(compression)),
                    labeled(
                        "Save to",
                        hstack((
                            text(bind(move |runtime| {
                                destination.with(runtime, |path| {
                                    path.as_deref()
                                        .map_or_else(|| NO_FILE.to_owned(), display_name)
                                })
                            }))
                            .grow(1.0)
                            .truncate(),
                            button("Choose…")
                                .disabled(choosing)
                                .on_click(move |cx, _| choose(cx)),
                        ))
                        .gap(GAP)
                        .items_center(),
                    ),
                )),
                actions((
                    cancel_button(open),
                    button("Create Image")
                        .variant(ButtonVariant::Primary)
                        .disabled(bind(move |runtime| {
                            destination.with(runtime, Option::is_none)
                        }))
                        .on_click(move |cx, _| create(cx)),
                )),
            )
        })
        .boxed()
    })
    .boxed()
}
