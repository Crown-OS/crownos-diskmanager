use crownui::prelude::Cx;

use crate::state::{DialogRequest, Store};

/// Keeps the window open while closing would affect a running operation and asks first.
pub fn guard_close(cx: &mut Cx, store: Store) {
    cx.on_close_requested(move |cx, event| {
        let busy = store
            .operations
            .with_untracked(cx, |operations| operations.interruptible().next().is_some());
        if busy {
            event.prevent_default();
            store.open_dialog(cx, DialogRequest::QuitWhileBusy);
        }
    });
}
