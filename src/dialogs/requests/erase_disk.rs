use std::sync::Arc;

use crownos_disks::Command;
use crownos_disks::command::{PartitionScheme, Target};
use crownos_disks::guard::{self, Action};
use crownos_disks::limits;
use crownos_disks::model::DiskId;
use crownui::kit::{ButtonOptions, ButtonVariant, ToggleItem, button, toggle_group};
use crownui::prelude::{AnyView, Cx, Listeners, Semantic, Styled, View, bind, component, vstack};

use crate::dialogs::context::SheetContext;
use crate::dialogs::fields::VolumeForm;
use crate::dialogs::sheets::layout::{
    actions, cancel_button, form_dialog, labeled, muted_text, sheet_title,
};
use crate::dialogs::sheets::{Confirmation, Step, confirm_sheet, gone_sheet, proceed, steps};

const SCHEMES: [(PartitionScheme, &str, &str); 2] = [
    (
        PartitionScheme::Gpt,
        "GPT",
        "Recommended for computers made in the last 15 years.",
    ),
    (
        PartitionScheme::Mbr,
        "MBR",
        "For older computers and some devices.",
    ),
];
const GAP: f32 = 6.0;

fn scheme_at(index: usize) -> (PartitionScheme, &'static str) {
    SCHEMES.get(index).map_or(
        (PartitionScheme::Gpt, SCHEMES[0].2),
        |(scheme, _, caption)| (*scheme, caption),
    )
}

fn scheme_picker(selected: crownui::prelude::Signal<usize>) -> impl View {
    vstack((
        toggle_group(
            SCHEMES.iter().map(|(_, label, _)| ToggleItem::text(*label)),
            selected,
        )
        .label("Partition scheme"),
        muted_text(bind(move |runtime| {
            scheme_at(selected.get(runtime)).1.to_owned()
        })),
    ))
    .gap(GAP)
}

pub fn erase_disk_sheet(context: SheetContext, disk_id: DiskId) -> AnyView {
    component(move |cx| {
        let inventory = context.store.inventory.with_untracked(cx, Arc::clone);
        let target = Target::Disk(disk_id.clone());
        let (Some(disk), Some(expected)) = (
            inventory.disk(&disk_id),
            guard::fingerprint(&inventory, &target),
        ) else {
            return gone_sheet(context, "The disk".to_owned()).boxed();
        };
        let open = context.sheet_open(cx);
        let step = cx.signal(Step::Form);
        let volume = VolumeForm::new(
            cx,
            &limits::default_filesystem(disk),
            String::new(),
            &inventory.capabilities,
        );
        let scheme = cx.signal(0);
        let tier = guard::confirmation_tier(&inventory, &target, Action::EraseDisk);
        let losses = guard::losses(&inventory, &target);
        let capabilities = inventory.capabilities.clone();
        let name = disk.name.clone();

        let submit = move |cx: &mut Cx| {
            let Some(spec) = volume.take_spec(cx) else {
                return;
            };
            let (scheme, _) = scheme_at(scheme.get(cx));
            let command = Command::EraseDisk {
                disk: disk_id.clone(),
                scheme,
                spec,
                expected: expected.clone(),
            };
            context.launch(cx, target.clone(), command);
        };
        let form = {
            let (name, submit) = (name.clone(), submit.clone());
            move || {
                let (name, submit, capabilities) =
                    (name.clone(), submit.clone(), capabilities.clone());
                form_dialog(open, move || {
                    let submit = submit.clone();
                    (
                        sheet_title(format!("Erase “{name}”")),
                        muted_text(
                            "Erasing removes everything on the disk and creates one new volume.",
                        ),
                        volume.view(&capabilities),
                        labeled("Scheme", scheme_picker(scheme)),
                        actions((
                            cancel_button(open),
                            button("Erase…")
                                .variant(ButtonVariant::Primary)
                                .disabled(bind(move |runtime| !volume.is_ready(runtime)))
                                .on_click(move |cx, _| proceed(cx, tier, step, submit.clone())),
                        )),
                    )
                })
            }
        };
        let confirm = move |cx: &mut Cx| {
            let confirmation = Confirmation {
                tier,
                title: format!("Erase “{name}”?"),
                message: format!("Everything on {name} will be permanently erased."),
                action: format!("Erase {name}"),
                target_name: name.clone(),
                losses: losses.clone(),
                notes: volume.warnings(cx),
            };
            confirm_sheet(open, confirmation, submit.clone())
        };
        steps(step, form, confirm).boxed()
    })
    .boxed()
}
