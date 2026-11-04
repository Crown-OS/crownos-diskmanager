use crownui::kit::use_toasts;
use crownui::prelude::{Styled, View, component, vstack};

use super::completion::completion;
use super::context::SheetContext;
use super::request::request_sheet;
use super::sheets::{error_sheet, progress_sheet};
use crate::state::Store;

/// Every sheet the app shows, driven by `store`: the one `store.dialog` asks for, the progress
/// of long operations, and results as toasts or the error sheet. Render it once
/// as the last child of the window-filling root, inside the toast host.
pub fn host(store: Store) -> impl View {
    component(move |cx| {
        let context = SheetContext::new(cx, store);
        let toasts = use_toasts(cx);
        let last_seen = cx.store(None);
        vstack((
            completion(context, toasts),
            vstack(progress_sheet(context)).pointer_events_auto(),
            vstack(request_sheet(context, last_seen)).pointer_events_auto(),
            vstack(error_sheet(context)).pointer_events_auto(),
        ))
        .absolute()
        .inset(0.0)
        .pointer_events_none()
    })
}
