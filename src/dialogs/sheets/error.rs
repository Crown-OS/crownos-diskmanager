use crownos_disks::UserError;
use crownui::kit::{ButtonOptions, ButtonVariant, button, dialog, icons};
use crownui::prelude::{
    AnyView, Listeners, Signal, Styled, View, bind, component, keyed, show, text, vstack,
};

use super::layout::{actions, muted_text, sheet_title};
use crate::dialogs::context::SheetContext;
use crate::util::nothing::nothing;

const GAP: f32 = 10.0;
const DETAILS_SIZE: f32 = 12.0;

/// The sheet explaining the last failure, with its raw details behind a disclosure.
pub fn error_sheet(context: SheetContext) -> impl View {
    keyed(
        move |runtime| context.error.get(runtime),
        move |_, error| match error {
            Some(error) => shown_error(context.error, error),
            None => nothing().boxed(),
        },
    )
}

fn shown_error(slot: Signal<Option<UserError>>, error: UserError) -> AnyView {
    component(move |cx| {
        let open = cx.signal(true);
        let expanded = cx.signal(false);
        cx.effect(move |runtime| {
            if !open.get(runtime) {
                slot.set(runtime, None);
            }
        });
        dialog(open, move || {
            (
                sheet_title(error.title.clone()),
                muted_text(error.message.clone()),
                error
                    .details
                    .clone()
                    .map(|details| details_disclosure(expanded, details)),
                actions((button("OK")
                    .variant(ButtonVariant::Primary)
                    .on_click(move |cx, _| open.set(cx, false)),)),
            )
        })
    })
    .boxed()
}

fn details_disclosure(expanded: Signal<bool>, details: String) -> impl View {
    vstack((
        button(bind(move |runtime| match expanded.get(runtime) {
            true => "Hide details".to_owned(),
            false => "Show details".to_owned(),
        }))
        .variant(ButtonVariant::Ghost)
        .leading_icon(icons::CHEVRON_RIGHT)
        .on_click(move |cx, _| expanded.update(cx, |expanded| *expanded = !*expanded)),
        show(
            expanded,
            move || text(details.clone()).text_size(DETAILS_SIZE),
            nothing,
        ),
    ))
    .gap(GAP)
}
