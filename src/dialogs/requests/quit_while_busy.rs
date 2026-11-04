use std::sync::Arc;

use crownos_disks::command::QuitImpact;
use crownos_disks::model::Inventory;
use crownui::kit::{ButtonOptions, ButtonVariant, button, dialog};
use crownui::prelude::{
    AnyView, Listeners, Semantic, Signal, Styled, View, ViewSeq, bind, component, vstack,
};

use crate::dialogs::context::SheetContext;
use crate::dialogs::sheets::layout::{actions, muted_text, sheet_title};
use crate::dialogs::target::target_name;
use crate::state::{Operations, RunningOperation};
use crate::util::sentence::lowercase_first;

const UNKNOWN_TARGET: &str = "the disk";
const GAP: f32 = 10.0;

#[derive(Clone)]
struct QuitPrompt {
    title: String,
    consequences: Vec<String>,
    cancels: bool,
}

fn consequence(inventory: &Inventory, operation: &RunningOperation) -> Option<String> {
    match operation.quit_impact {
        QuitImpact::Harmless => None,
        QuitImpact::LosesImage => Some("The image won't be saved.".to_owned()),
        QuitImpact::LeavesTargetUnusable => {
            let name = target_name(inventory, &operation.target)
                .unwrap_or_else(|| UNKNOWN_TARGET.to_owned());
            Some(format!("Stopping now can leave {name} unusable."))
        }
        QuitImpact::FinishesInBackground => Some(format!(
            "{} will keep running in the background and finish on its own.",
            operation.label
        )),
        QuitImpact::StopsPartway => Some(format!(
            "Quitting now stops {} before it finishes. Disk Manager will show how to finish or \
             recover the next time it opens.",
            lowercase_first(&operation.label)
        )),
    }
}

impl QuitPrompt {
    fn new(inventory: &Inventory, operations: &Operations) -> Self {
        let running: Vec<&RunningOperation> = operations
            .interruptible()
            .map(|(_, operation)| operation)
            .collect();
        let title = match running.as_slice() {
            [operation] => format!(
                "Quit while {} is in progress?",
                lowercase_first(&operation.label)
            ),
            several => format!("Quit while {} operations are in progress?", several.len()),
        };
        Self {
            title,
            consequences: running
                .iter()
                .filter_map(|operation| consequence(inventory, operation))
                .collect(),
            cancels: operations.stopping_with_app().next().is_some(),
        }
    }
}

/// Asks before the window closes on operations that are still running: image copies are
/// cancelled first, udisksd jobs are left to finish on their own.
pub fn quit_while_busy_sheet(context: SheetContext) -> AnyView {
    component(move |cx| {
        let store = context.store;
        let inventory = store.inventory.with_untracked(cx, Arc::clone);
        let prompt = store
            .operations
            .with_untracked(cx, |operations| QuitPrompt::new(&inventory, operations));
        let open = context.sheet_open(cx);
        cx.effect(move |runtime| {
            if !open.get(runtime) {
                store.quitting.set(runtime, false);
            }
        });
        dialog(open, move || content(context, open, prompt.clone()))
    })
    .boxed()
}

fn content(context: SheetContext, open: Signal<bool>, prompt: QuitPrompt) -> impl ViewSeq {
    let store = context.store;
    let quit = match prompt.cancels {
        true => button(bind(move |runtime| match store.quitting.get(runtime) {
            true => "Cancelling…".to_owned(),
            false => "Cancel and Quit".to_owned(),
        }))
        .variant(ButtonVariant::Destructive)
        .disabled(store.quitting),
        false => button("Quit").variant(ButtonVariant::Primary),
    }
    .on_click(move |cx, _| store.quit(cx));
    let consequences: Vec<AnyView> = prompt
        .consequences
        .into_iter()
        .map(|line| muted_text(line).boxed())
        .collect();
    (
        sheet_title(prompt.title),
        vstack(consequences).gap(GAP),
        actions((
            button("Keep Working")
                .variant(ButtonVariant::Secondary)
                .on_click(move |cx, _| open.set(cx, false)),
            quit,
        )),
    )
}
