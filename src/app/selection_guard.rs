use crownui::prelude::Cx;

use crate::state::{Selection, Store};
use crate::util::selection::{selected_disk, still_present};

pub fn keep_selection_valid(cx: &mut Cx, store: Store) {
    cx.effect(move |runtime| {
        let selection = store.selection.get(runtime);
        let fallback = store.inventory.with(runtime, |inventory| {
            if still_present(inventory, &selection) {
                return None;
            }
            Some(match selected_disk(&selection) {
                Some(disk) if inventory.disk(disk).is_some() => Selection::Disk(disk.clone()),
                _ => Selection::Overview,
            })
        });
        if let Some(fallback) = fallback {
            runtime.untrack(|runtime| store.selection.set(runtime, fallback));
        }
    });
}
