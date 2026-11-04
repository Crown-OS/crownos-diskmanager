use crownui::kit::{ButtonOptions, ButtonVariant, button, dialog};
use crownui::prelude::{Listeners, View, component};

use super::layout::{actions, muted_text, sheet_title};
use crate::dialogs::context::SheetContext;

/// Replaces a sheet whose disk or volume was unplugged while it was open.
pub fn gone_sheet(context: SheetContext, name: String) -> impl View {
    component(move |cx| {
        let open = context.sheet_open(cx);
        dialog(open, move || {
            (
                sheet_title(format!("{name} is no longer connected")),
                muted_text("Nothing was changed. Reconnect it and try again."),
                actions((button("OK")
                    .variant(ButtonVariant::Primary)
                    .on_click(move |cx, _| open.set(cx, false)),)),
            )
        })
    })
}
