use std::sync::Arc;

use crownos_disks::Command;
use crownos_disks::command::Target;
use crownos_disks::model::{BlockId, FilesystemKind};
use crownui::kit::{ButtonOptions, ButtonVariant, button};
use crownui::prelude::{AnyView, Cx, Listeners, Runtime, Semantic, View, bind, component};

use crate::dialogs::context::SheetContext;
use crate::dialogs::fields::LabelField;
use crate::dialogs::sheets::gone_sheet;
use crate::dialogs::sheets::layout::{actions, cancel_button, form_dialog, sheet_title};
use crate::dialogs::target::Resolved;

const UNNAMED_FILESYSTEM: &str = "";

pub fn rename_sheet(context: SheetContext, block: BlockId) -> AnyView {
    component(move |cx| {
        let inventory = context.store.inventory.with_untracked(cx, Arc::clone);
        let Some(resolved) = Resolved::find(&inventory, &Target::Block(block.clone())) else {
            return gone_sheet(context, "The volume".to_owned()).boxed();
        };
        let volume = resolved.content().and_then(|content| content.volume());
        let filesystem = volume.map_or_else(
            || FilesystemKind::Other(UNNAMED_FILESYSTEM.into()),
            |volume| volume.filesystem.clone(),
        );
        let current = volume
            .map(|volume| volume.label.clone())
            .unwrap_or_default();
        let name = resolved.name().into_owned();
        let open = context.sheet_open(cx);
        let label = LabelField::new(cx, current.clone(), move |_| filesystem.clone());
        let ready = move |runtime: &mut Runtime| {
            label.is_valid(runtime) && label.value.with(runtime, |value| *value != current)
        };
        let can_rename = ready.clone();
        let rename = move |cx: &mut Cx| {
            if !can_rename(cx) {
                return;
            }
            let label = label.value.get(cx);
            context.store.send(
                cx,
                Command::Rename {
                    block: block.clone(),
                    label,
                },
            );
            context.close(cx);
        };
        form_dialog(open, move || {
            let (submit, click, ready) = (rename.clone(), rename.clone(), ready.clone());
            (
                sheet_title(format!("Rename “{name}”")),
                label.view_with_submit("Name", submit),
                actions((
                    cancel_button(open),
                    button("Rename")
                        .variant(ButtonVariant::Primary)
                        .disabled(bind(move |runtime| !ready(runtime)))
                        .on_click(move |cx, _| click(cx)),
                )),
            )
        })
        .boxed()
    })
    .boxed()
}
