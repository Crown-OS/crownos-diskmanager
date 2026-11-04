use crownos_disks::Command;
use crownui::prelude::{AnyView, View, component};

use super::image_file::pick_image;
use crate::dialogs::context::SheetContext;
use crate::util::nothing::nothing;
use crate::util::portal::FileChoice;

/// No sheet of its own: the desktop's file chooser, then the image opens.
pub fn open_image_request(context: SheetContext) -> AnyView {
    component(move |cx| {
        pick_image(
            cx,
            context,
            "Open Disk Image",
            FileChoice::Open,
            move |cx, path| {
                if let Some(path) = path {
                    context.store.send(cx, Command::OpenImage(path));
                }
                context.close(cx);
            },
        );
        nothing()
    })
    .boxed()
}
