use std::sync::Arc;

use crownos_disks::Command;
use crownos_disks::command::Target;
use crownos_disks::guard::{self, Action};
use crownos_disks::model::BlockId;
use crownui::prelude::{AnyView, Cx, View, component};

use crate::dialogs::context::SheetContext;
use crate::dialogs::sheets::{Confirmation, confirm_sheet, gone_sheet};
use crate::dialogs::target::Resolved;

pub fn delete_sheet(context: SheetContext, block: BlockId) -> AnyView {
    component(move |cx| {
        let inventory = context.store.inventory.with_untracked(cx, Arc::clone);
        let target = Target::Block(block.clone());
        let (Some(resolved), Some(expected)) = (
            Resolved::find(&inventory, &target),
            guard::fingerprint(&inventory, &target),
        ) else {
            return gone_sheet(context, "The partition".to_owned()).boxed();
        };
        let name = resolved.name().into_owned();
        let open = context.sheet_open(cx);
        let confirmation = Confirmation {
            tier: guard::confirmation_tier(&inventory, &target, Action::DeletePartition),
            title: format!("Delete “{name}”?"),
            message: format!("{name} and everything on it will be deleted."),
            action: format!("Delete {name}"),
            target_name: name,
            losses: guard::losses(&inventory, &target),
            notes: Vec::new(),
        };
        let submit = move |cx: &mut Cx| {
            let command = Command::DeletePartition {
                block: block.clone(),
                expected: expected.clone(),
            };
            context.launch(cx, target.clone(), command);
        };
        confirm_sheet(open, confirmation, submit).boxed()
    })
    .boxed()
}
