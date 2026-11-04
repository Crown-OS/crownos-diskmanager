use crownos_disks::Command;
use crownos_disks::command::OperationId;
use crownui::ext::{Key, NamedKey};
use crownui::kit::{
    ButtonOptions, ButtonVariant, DialogOptions, button, dialog, progress, spinner,
};
use crownui::prelude::{
    Listeners, Runtime, Semantic, Styled, View, bind, component, keyed, show, vstack,
};

use super::layout::{actions, muted_text, sheet_title};
use crate::dialogs::context::SheetContext;
use crate::state::RunningOperation;
use crate::util::nothing::nothing;
use crate::util::remaining_time::rate_and_remaining;

const GAP: f32 = 10.0;
const BAR_WIDTH: f32 = 400.0;

fn shown_operation(context: SheetContext, runtime: &mut Runtime) -> Option<OperationId> {
    runtime.with2(
        context.store.operations,
        context.launched,
        |operations, launched| {
            operations
                .running
                .iter()
                .find(|(_, operation)| {
                    operation.cancelable
                        || operation.progress.fraction.is_some()
                        || launched.contains(&operation.target)
                })
                .map(|(id, _)| *id)
        },
    )
}

/// The non-dismissable sheet of the long operation running now, if any.
pub fn progress_sheet(context: SheetContext) -> impl View {
    keyed(
        move |runtime| shown_operation(context, runtime),
        move |_, shown| match shown {
            Some(id) => operation_sheet(context, id).boxed(),
            None => nothing().boxed(),
        },
    )
}

fn read(context: SheetContext, runtime: &mut Runtime, id: OperationId) -> Option<RunningOperation> {
    context
        .store
        .operations
        .with(runtime, |operations| operations.running.get(&id).cloned())
}

fn operation_sheet(context: SheetContext, id: OperationId) -> impl View {
    component(move |cx| {
        let open = cx.signal(true);
        let cancelling = cx.signal(false);
        let field = move |pick: fn(&RunningOperation) -> String| {
            bind(move |runtime| {
                read(context, runtime, id)
                    .map(|operation| pick(&operation))
                    .unwrap_or_default()
            })
        };
        let fraction = move |runtime: &mut Runtime| {
            read(context, runtime, id).and_then(|operation| operation.progress.fraction)
        };
        let cancelable = move |runtime: &mut Runtime| {
            read(context, runtime, id).is_some_and(|operation| operation.cancelable)
        };
        dialog(open, move || {
            vstack((
                sheet_title(field(|operation| operation.label.clone())),
                muted_text(field(|operation| operation.phase.clone())),
                show(
                    bind(move |runtime| fraction(runtime).is_some()),
                    move || {
                        progress(bind(move |runtime| fraction(runtime).unwrap_or_default()))
                            .w(BAR_WIDTH)
                    },
                    spinner,
                ),
                muted_text(field(|operation| rate_and_remaining(&operation.progress))),
                show(
                    bind(cancelable),
                    move || {
                        actions((button(bind(move |runtime| match cancelling.get(runtime) {
                            true => "Cancelling…".to_owned(),
                            false => "Cancel".to_owned(),
                        }))
                        .variant(ButtonVariant::Secondary)
                        .disabled(cancelling)
                        .on_click(move |cx, _| {
                            cancelling.set(cx, true);
                            context.store.send(cx, Command::Cancel(id));
                        }),))
                    },
                    nothing,
                ),
            ))
            .gap(GAP)
            .focusable(true)
            .label("Operation in progress")
            .on_key_down(|_, event| {
                if event.key == Key::Named(NamedKey::Escape) {
                    event.prevent_default();
                    event.stop_propagation();
                }
            })
        })
        .dismiss_on_scrim(false)
    })
}
