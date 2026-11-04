use std::path::PathBuf;

use crownos_disks::model::{ByteSize, Content, Disk, Encryption, LuksVersion, Partition, Region};
use crownui::kit::{ButtonOptions, ButtonSize, ButtonVariant, button, card, icons};
use crownui::prelude::{
    Cx, Listeners, Memo, Role, Semantic, Styled, View, hstack, keyed, spacer, vstack,
};

use crate::components::{UsageShare, body, caption, headline, usage_meter};
use crate::state::{Selection, Store};
use crate::util::nothing::nothing;
use crate::util::open_in_files::open_in_files;
use crate::util::region_text::{content_summary, free_title, partition_title};
use crate::util::selection::region_selection;

const CARD_PADDING: f32 = 18.0;
const CARD_GAP: f32 = 12.0;
const TEXT_GAP: f32 = 2.0;
const FREE_HINT: &str = "Use New Partition in the toolbar to make this space usable.";
const UNALLOCATED_HINT: &str =
    "This disk is empty. Use Erase in the toolbar to prepare it for use.";

#[derive(Debug, Clone, PartialEq)]
enum Shown {
    Partition {
        partition: Box<Partition>,
        color: usize,
    },
    Free {
        size: ByteSize,
        has_table: bool,
    },
}

pub fn region_card(store: Store, disk: Memo<Option<Disk>>) -> impl View {
    keyed(
        move |runtime| {
            let selection = store.selection.get(runtime);
            disk.with(runtime, |disk| {
                disk.as_ref()
                    .and_then(|disk| shown_region(disk, &selection))
            })
        },
        move |cx, shown| match shown {
            Some(shown) => region_view(cx, shown).boxed(),
            None => nothing().boxed(),
        },
    )
    .flex_col()
}

fn shown_region(disk: &Disk, selection: &Selection) -> Option<Shown> {
    let regions = &disk.layout;
    let is_partition = |region: &Region| matches!(region, Region::Partition(_));
    let index = match selection {
        Selection::Disk(_) => regions
            .iter()
            .position(is_partition)
            .or((!regions.is_empty()).then_some(0)),
        _ => regions
            .iter()
            .position(|region| region_selection(&disk.id, region) == *selection),
    }?;
    let color = regions[..index]
        .iter()
        .filter(|region| is_partition(region))
        .count();
    Some(match &regions[index] {
        Region::Partition(partition) => Shown::Partition {
            partition: Box::new(partition.clone()),
            color,
        },
        Region::Free { size, .. } => Shown::Free {
            size: *size,
            has_table: disk.table.is_some(),
        },
    })
}

fn region_view(_cx: &mut Cx, shown: Shown) -> impl View + use<> {
    let content = match shown {
        Shown::Free { size, has_table } => vstack((
            headline(free_title(has_table)),
            caption(size.to_string()),
            body(if has_table {
                FREE_HINT
            } else {
                UNALLOCATED_HINT
            }),
        ))
        .gap(CARD_GAP)
        .boxed(),
        Shown::Partition { partition, color } => partition_view(&partition, color).boxed(),
    };
    card(content)
        .p(CARD_PADDING)
        .self_stretch()
        .role(Role::Group)
        .label("Selected region")
}

fn partition_view(partition: &Partition, color: usize) -> impl View + use<> {
    let name = partition_title(partition).into_owned();
    let volume = partition.content.volume();
    let mounted_at = volume.and_then(|volume| volume.mount_points.first().cloned());
    let facts = [
        Some(content_summary(&partition.content).into_owned()),
        Some(partition.size.to_string()),
        volume.map(|_| match &mounted_at {
            Some(path) => format!("mounted at {}", path.display()),
            None => "not mounted".to_owned(),
        }),
        encryption_fact(&partition.content),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" · ");
    let usage = volume.and_then(|volume| volume.usage).map(|usage| {
        usage_meter(
            format!("{name} usage"),
            vec![UsageShare {
                used: usage.used,
                color,
            }],
            usage.total,
        )
    });
    vstack((
        hstack((
            vstack((headline(name), caption(facts)))
                .gap(TEXT_GAP)
                .min_w(0.0),
            spacer(),
            open_button(mounted_at),
        ))
        .items_center(),
        usage,
    ))
    .gap(CARD_GAP)
}

fn encryption_fact(content: &Content) -> Option<String> {
    let Content::Encrypted(Encryption {
        version, unlocked, ..
    }) = content
    else {
        return None;
    };
    let version = match version {
        LuksVersion::Luks1 => "LUKS1",
        LuksVersion::Luks2 => "LUKS2",
    };
    let state = if unlocked.is_some() {
        "unlocked"
    } else {
        "locked"
    };
    Some(format!("{version}, {state}"))
}

fn open_button(mounted_at: Option<PathBuf>) -> Option<impl View> {
    mounted_at.map(|path| {
        button("Open in Files")
            .leading_icon(icons::FOLDER_OPEN)
            .variant(ButtonVariant::Secondary)
            .control_size(ButtonSize::Small)
            .on_click(move |_, _| _ = open_in_files(&path))
    })
}
