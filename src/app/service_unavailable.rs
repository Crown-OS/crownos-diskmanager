use crownos_disks::UserError;
use crownui::kit::{icon_muted, icons};
use crownui::prelude::{Role, Semantic, Styled, View, vstack};

use crate::components::{body, headline};

const ICON_SIZE: f32 = 40.0;
const GAP: f32 = 12.0;
const MAX_TEXT_WIDTH: f32 = 420.0;
pub const SERVICE_UNAVAILABLE_TITLE: &str = "Storage service isn't running";

pub fn service_unavailable(error: UserError) -> impl View {
    vstack((
        icon_muted(icons::CIRCLE_ALERT).size(ICON_SIZE),
        headline(SERVICE_UNAVAILABLE_TITLE),
        vstack(body(error.message))
            .max_w(MAX_TEXT_WIDTH)
            .items_center(),
    ))
    .gap(GAP)
    .flex_1()
    .items_center()
    .justify_center()
    .role(Role::Group)
    .label(SERVICE_UNAVAILABLE_TITLE)
}
