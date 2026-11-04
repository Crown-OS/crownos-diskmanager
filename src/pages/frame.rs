use crownui::prelude::{IntoProp, Semantic, Styled, View, ViewSeq, scroll_view, vstack};

pub const PAGE_PADDING: f32 = 32.0;
pub const CARD_GAP: f32 = 24.0;

pub fn page(name: impl IntoProp<String>, cards: impl ViewSeq + 'static) -> impl View {
    scroll_view(vstack(cards).gap(CARD_GAP).p(PAGE_PADDING))
        .flex_1()
        .label(name)
}
