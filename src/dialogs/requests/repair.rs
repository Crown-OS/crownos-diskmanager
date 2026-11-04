use std::sync::Arc;

use crownos_disks::Command;
use crownos_disks::command::Target;
use crownos_disks::guard::ConfirmationTier;
use crownos_disks::model::BlockId;
use crownui::prelude::{AnyView, Cx, View, component};

use crate::dialogs::context::SheetContext;
use crate::dialogs::sheets::{Confirmation, confirm_sheet, gone_sheet};
use crate::dialogs::target::Resolved;

const SLOW_NOTE: &str = "This can take a while on large drives.";

pub fn repair_message(name: &str, mounted: bool) -> String {
    match mounted {
        true => format!("{name} will be unmounted while it's checked and repaired. {SLOW_NOTE}"),
        false => format!("{name} will be checked and repaired. {SLOW_NOTE}"),
    }
}

pub fn repair_sheet(context: SheetContext, block: BlockId) -> AnyView {
    component(move |cx| {
        let inventory = context.store.inventory.with_untracked(cx, Arc::clone);
        let target = Target::Block(block.clone());
        let Some(resolved) = Resolved::find(&inventory, &target) else {
            return gone_sheet(context, "The volume".to_owned()).boxed();
        };
        let name = resolved.name().into_owned();
        let mounted = resolved
            .content()
            .and_then(|content| content.volume())
            .is_some_and(|volume| volume.is_mounted());
        let open = context.sheet_open(cx);
        let confirmation = Confirmation {
            tier: ConfirmationTier::Simple,
            title: format!("Check & Repair {name}?"),
            message: repair_message(&name, mounted),
            action: "Check & Repair".to_owned(),
            target_name: name,
            losses: Vec::new(),
            notes: Vec::new(),
        };
        let submit =
            move |cx: &mut Cx| context.launch(cx, target.clone(), Command::Repair(block.clone()));
        confirm_sheet(open, confirmation, submit).boxed()
    })
    .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_unmount_sentence_only_appears_for_mounted_volumes() {
        assert_eq!(
            repair_message("Root", true),
            "Root will be unmounted while it's checked and repaired. This can take a while on large drives."
        );
        assert_eq!(
            repair_message("Root", false),
            "Root will be checked and repaired. This can take a while on large drives."
        );
    }
}
