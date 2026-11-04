use crownos_disks::command::Target;
use crownui::kit::{Toast, Toasts, icons};
use crownui::prelude::{Cx, View, keyed};

use super::context::SheetContext;
use crate::state::FinishedOperation;
use crate::util::nothing::nothing;

fn forget_launch(cx: &mut Cx, context: SheetContext, target: &Target) {
    context.launched.update(cx, |launched| {
        if let Some(index) = launched.iter().position(|launched| launched == target) {
            launched.remove(index);
        }
    });
}

fn settle(cx: &mut Cx, context: SheetContext, toasts: Toasts, finished: FinishedOperation) {
    if let Some(target) = &finished.target {
        forget_launch(cx, context, target);
    }
    let awaited = finished.target.is_some()
        && context
            .awaiting
            .with_untracked(cx, |awaiting| *awaiting == finished.target);
    if awaited {
        context.awaiting.set(cx, None);
    }
    match finished.result {
        Ok(completion) => {
            if awaited {
                context.close(cx);
            }
            let message = match completion.message.is_empty() {
                true => finished.label,
                false => completion.message,
            };
            if !message.is_empty() {
                toasts.show_with(cx, Toast::new(message).icon(icons::CIRCLE_CHECK));
            }
        }
        Err(error) if error.silent => {}
        Err(error) if awaited => context.inline_error.set(cx, Some(error)),
        Err(error) => context.report(cx, error),
    }
}

/// Turns each finished operation into a toast, an inline error for the sheet
/// waiting on it, or the error sheet.
pub fn completion(context: SheetContext, toasts: Toasts) -> impl View {
    keyed(
        move |runtime| {
            context.store.operations.with(runtime, |operations| {
                operations.finished.as_ref().map(|finished| finished.id)
            })
        },
        move |cx, id| {
            let finished = context
                .store
                .operations
                .with_untracked(cx, |operations| operations.finished.clone());
            if let Some(finished) = finished.filter(|finished| Some(finished.id) == id) {
                settle(cx, context, toasts, finished);
            }
            nothing()
        },
    )
}
