use crownui::prelude::{AnyView, Runtime, Store as Slot, View, keyed};

use super::context::SheetContext;
use super::requests::{
    create_image_sheet, delete_sheet, erase_disk_sheet, format_sheet, new_partition_sheet,
    open_image_request, quit_while_busy_sheet, rename_sheet, repair_sheet, resize_sheet,
    restore_image_sheet, unlock_sheet,
};
use super::sheets::gone_sheet;
use super::target::{request_target, target_name};
use crate::state::DialogRequest;
use crate::util::nothing::nothing;

const UNKNOWN_TARGET: &str = "The disk";

#[derive(Debug, Clone, PartialEq, Eq)]
enum Presence {
    Present,
    Gone,
}

/// The name of the request's target, remembered while it is present so the note can name it
/// after it is gone.
pub type LastSeen = Slot<Option<(DialogRequest, String)>>;

fn presence(
    context: SheetContext,
    runtime: &mut Runtime,
    request: &DialogRequest,
    last_seen: LastSeen,
) -> Presence {
    let Some(target) = request_target(request) else {
        return Presence::Present;
    };
    match context
        .store
        .inventory
        .with(runtime, |inventory| target_name(inventory, &target))
    {
        Some(name) => {
            last_seen.update(runtime, |last| *last = Some((request.clone(), name)));
            Presence::Present
        }
        None => Presence::Gone,
    }
}

fn last_name(runtime: &Runtime, last_seen: LastSeen, request: &DialogRequest) -> String {
    last_seen.with(runtime, |last| match last {
        Some((seen, name)) if seen == request => name.clone(),
        _ => UNKNOWN_TARGET.to_owned(),
    })
}

fn sheet(context: SheetContext, request: DialogRequest) -> AnyView {
    match request {
        DialogRequest::EraseDisk(disk) => erase_disk_sheet(context, disk),
        DialogRequest::NewPartition { disk, offset, size } => {
            new_partition_sheet(context, disk, offset, size)
        }
        DialogRequest::Format(block) => format_sheet(context, block),
        DialogRequest::Resize(block) => resize_sheet(context, block),
        DialogRequest::Rename(block) => rename_sheet(context, block),
        DialogRequest::Unlock(block) => unlock_sheet(context, block),
        DialogRequest::Delete(block) => delete_sheet(context, block),
        DialogRequest::Repair(block) => repair_sheet(context, block),
        DialogRequest::CreateImage(target) => create_image_sheet(context, target),
        DialogRequest::RestoreImage(target) => restore_image_sheet(context, target),
        DialogRequest::OpenImage => open_image_request(context),
        DialogRequest::QuitWhileBusy => quit_while_busy_sheet(context),
    }
}

/// The sheet `store.dialog` asks for; it turns into a short note when its disk is unplugged.
pub fn request_sheet(context: SheetContext, last_seen: LastSeen) -> impl View {
    keyed(
        move |runtime| {
            let request = context.store.dialog.get(runtime)?;
            let presence = presence(context, runtime, &request, last_seen);
            Some((request, presence))
        },
        move |cx, shown| match shown {
            None => nothing().boxed(),
            Some((request, Presence::Present)) => sheet(context, request),
            Some((request, Presence::Gone)) => {
                gone_sheet(context, last_name(cx, last_seen, &request)).boxed()
            }
        },
    )
}
