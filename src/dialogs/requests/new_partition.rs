use std::sync::Arc;

use crownos_disks::Command;
use crownos_disks::command::Target;
use crownos_disks::guard::{self, Action};
use crownos_disks::limits;
use crownos_disks::model::{ByteSize, DiskId};
use crownui::kit::{ButtonOptions, ButtonVariant, button};
use crownui::prelude::{AnyView, Cx, Listeners, Semantic, View, bind, component};

use crate::dialogs::context::SheetContext;
use crate::dialogs::fields::{ByteSizeField, VolumeForm};
use crate::dialogs::sheets::layout::{
    actions, cancel_button, form_dialog, labeled, muted_text, sheet_title,
};
use crate::dialogs::sheets::{Confirmation, Step, confirm_sheet, gone_sheet, proceed, steps};
use crate::util::byte_size_parse::snap_size;

pub fn new_partition_sheet(
    context: SheetContext,
    disk_id: DiskId,
    offset: ByteSize,
    space: ByteSize,
) -> AnyView {
    component(move |cx| {
        let inventory = context.store.inventory.with_untracked(cx, Arc::clone);
        let target = Target::Disk(disk_id.clone());
        let (Some(disk), Some(expected)) = (inventory.disk(&disk_id), guard::fingerprint(&inventory, &target)) else {
            return gone_sheet(context, "The disk".to_owned()).boxed();
        };
        let open = context.sheet_open(cx);
        let step = cx.signal(Step::Form);
        let size = ByteSizeField::new(cx, snap_size(space, ByteSize::MIB, space), ByteSize::MIB, space);
        let volume = VolumeForm::new(cx, &limits::default_filesystem(disk), String::new(), &inventory.capabilities);
        let tier = guard::confirmation_tier(&inventory, &target, Action::CreatePartition);
        let capabilities = inventory.capabilities.clone();
        let name = disk.name.clone();

        let submit = move |cx: &mut Cx| {
            let size = size.size.get(cx);
            let partition_name = volume.label_text(cx);
            let Some(spec) = volume.take_spec(cx) else {
                return;
            };
            let command = Command::CreatePartition {
                disk: disk_id.clone(),
                offset,
                size,
                name: partition_name,
                spec,
                expected: expected.clone(),
            };
            context.launch(cx, target.clone(), command);
        };
        let form = {
            let (name, submit) = (name.clone(), submit.clone());
            move || {
                let (name, submit, capabilities) = (name.clone(), submit.clone(), capabilities.clone());
                form_dialog(open, move || {
                    let submit = submit.clone();
                    (
                        sheet_title("New Partition"),
                        muted_text(format!("Creates a partition in the free space on {name}.")),
                        labeled("Size", size.view()),
                        volume.view(&capabilities),
                        actions((
                            cancel_button(open),
                            button("Create…")
                                .variant(ButtonVariant::Primary)
                                .disabled(bind(move |runtime| !volume.is_ready(runtime)))
                                .on_click(move |cx, _| proceed(cx, tier, step, submit.clone())),
                        )),
                    )
                })
            }
        };
        let confirm = move |cx: &mut Cx| {
            let label = volume.label_text(cx);
            let named = match label.is_empty() {
                true => String::new(),
                false => format!(" named “{label}”"),
            };
            let confirmation = Confirmation {
                tier,
                title: format!("Create a partition on “{name}”?"),
                message: format!(
                    "A {} {} partition{named} will be created in the free space. Nothing else on the disk changes.",
                    size.size.get(cx),
                    volume.filesystem_name(cx)
                ),
                action: "Create Partition".to_owned(),
                target_name: name.clone(),
                losses: Vec::new(),
                notes: volume.warnings(cx),
            };
            confirm_sheet(open, confirmation, submit.clone())
        };
        steps(step, form, confirm).boxed()
    })
    .boxed()
}
