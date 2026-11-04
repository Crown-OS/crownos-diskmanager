use crownui::prelude::{Styled, View, vstack};

pub fn nothing() -> impl View {
    vstack(()).display_none()
}
