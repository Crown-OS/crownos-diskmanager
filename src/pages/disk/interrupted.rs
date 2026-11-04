use crownos_disks::Command;
use crownos_disks::journal::{JournalEntry, JournalOperation};
use crownos_disks::model::DiskId;
use crownui::kit::{
    ButtonOptions, ButtonSize, ButtonVariant, CalloutOptions, Tone, button, callout,
};
use crownui::prelude::{AnyView, Listeners, Styled, View, hstack, keyed, vstack};

use crate::state::Store;

const ENTRY_GAP: f32 = 8.0;
const BUTTON_GAP: f32 = 8.0;

pub fn interrupted(store: Store, disk: DiskId) -> impl View {
    keyed(
        move |runtime| {
            store.interrupted.with(runtime, |entries| {
                entries
                    .iter()
                    .filter(|entry| entry.disk == disk)
                    .cloned()
                    .collect::<Vec<_>>()
            })
        },
        move |_, entries: Vec<JournalEntry>| {
            let notices: Vec<AnyView> = entries
                .into_iter()
                .map(|entry| notice(store, entry).boxed())
                .collect();
            vstack(notices).gap(ENTRY_GAP)
        },
    )
    .flex_col()
}

fn notice(store: Store, entry: JournalEntry) -> impl View {
    let resumable = matches!(
        entry.operation,
        JournalOperation::Shrink { .. } | JournalOperation::Grow { .. }
    );
    let finish = resumable.then(|| {
        let entry = entry.clone();
        button("Finish Resizing")
            .variant(ButtonVariant::Primary)
            .control_size(ButtonSize::Small)
            .on_click(move |cx, _| store.send(cx, Command::ResumeInterrupted(entry.clone())))
    });
    let title = format!("{} needs attention", entry.target_name);
    let advice = entry.recovery_advice();
    let dismiss = button("Dismiss")
        .variant(ButtonVariant::Secondary)
        .control_size(ButtonSize::Small)
        .on_click(move |cx, _| store.send(cx, Command::DismissInterrupted(entry.clone())));
    vstack((
        callout(Tone::Warning, title).body(advice),
        hstack((finish, dismiss)).gap(BUTTON_GAP),
    ))
    .gap(ENTRY_GAP)
}
