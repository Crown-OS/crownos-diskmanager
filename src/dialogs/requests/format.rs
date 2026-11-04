use std::sync::Arc;

use crownos_disks::Command;
use crownos_disks::command::Target;
use crownos_disks::guard::{self, Action};
use crownos_disks::limits;
use crownos_disks::model::{BlockId, FilesystemKind};
use crownui::kit::{ButtonOptions, ButtonVariant, button};
use crownui::prelude::{AnyView, Cx, Listeners, Semantic, View, bind, component};

use crate::dialogs::context::SheetContext;
use crate::dialogs::fields::VolumeForm;
use crate::dialogs::sheets::layout::{
    actions, cancel_button, form_dialog, muted_text, sheet_title,
};
use crate::dialogs::sheets::{Confirmation, Step, confirm_sheet, gone_sheet, proceed, steps};
use crate::dialogs::target::Resolved;

pub fn format_sheet(context: SheetContext, block: BlockId) -> AnyView {
    component(move |cx| {
        let inventory = context.store.inventory.with_untracked(cx, Arc::clone);
        let target = Target::Block(block.clone());
        let (Some(resolved), Some(expected)) = (
            Resolved::find(&inventory, &target),
            guard::fingerprint(&inventory, &target),
        ) else {
            return gone_sheet(context, "The volume".to_owned()).boxed();
        };
        let current = resolved.content().and_then(|content| content.volume());
        let preferred = current
            .map(|volume| volume.filesystem.clone())
            .filter(|filesystem| FilesystemKind::FORMATTABLE.contains(filesystem))
            .unwrap_or_else(|| limits::default_filesystem(resolved.disk()));
        let initial_label = current
            .map(|volume| volume.label.clone())
            .unwrap_or_default();
        let open = context.sheet_open(cx);
        let step = cx.signal(Step::Form);
        let volume = VolumeForm::new(cx, &preferred, initial_label, &inventory.capabilities);
        let tier = guard::confirmation_tier(&inventory, &target, Action::Format);
        let losses = guard::losses(&inventory, &target);
        let capabilities = inventory.capabilities.clone();
        let name = resolved.name().into_owned();

        let submit = move |cx: &mut Cx| {
            let Some(spec) = volume.take_spec(cx) else {
                return;
            };
            let command = Command::Format {
                block: block.clone(),
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
                        sheet_title(format!("Format “{name}”")),
                        muted_text("Formatting removes everything on this volume."),
                        volume.view(&capabilities),
                        actions((
                            cancel_button(open),
                            button("Format…")
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
                title: format!("Format “{name}”?"),
                message: format!("Everything on {name} will be permanently erased."),
                action: format!("Format {name}"),
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
