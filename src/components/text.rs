use crownui::kit::{INTER_FAMILY, TEXT_SIZE, use_theme};
use crownui::prelude::{IntoProp, Styled, View, component, text};

use crate::util::tint::tint;

const TITLE_SIZE: f32 = 22.0;
const HEADLINE_SIZE: f32 = 17.0;
const CAPTION_SIZE: f32 = 12.0;

pub fn title(content: impl IntoProp<String> + 'static) -> impl View {
    component(move |cx| {
        let theme = use_theme(cx);
        text(content)
            .text_size(TITLE_SIZE)
            .font_semibold()
            .font_family(INTER_FAMILY)
            .truncate()
            .text_color(tint(theme, |theme| theme.text.primary))
    })
}

pub fn headline(content: impl IntoProp<String> + 'static) -> impl View {
    component(move |cx| {
        let theme = use_theme(cx);
        text(content)
            .text_size(HEADLINE_SIZE)
            .font_semibold()
            .font_family(INTER_FAMILY)
            .truncate()
            .text_color(tint(theme, |theme| theme.text.primary))
    })
}

pub fn body(content: impl IntoProp<String> + 'static) -> impl View {
    component(move |cx| {
        let theme = use_theme(cx);
        text(content)
            .text_size(TEXT_SIZE)
            .font_family(INTER_FAMILY)
            .text_color(tint(theme, |theme| theme.text.body))
    })
}

pub fn caption(content: impl IntoProp<String> + 'static) -> impl View {
    component(move |cx| {
        let theme = use_theme(cx);
        text(content)
            .text_size(CAPTION_SIZE)
            .font_family(INTER_FAMILY)
            .truncate()
            .text_color(tint(theme, |theme| theme.text.muted))
    })
}

pub fn value_text(content: impl IntoProp<String> + 'static) -> impl View {
    component(move |cx| {
        let theme = use_theme(cx);
        text(content)
            .text_size(TEXT_SIZE)
            .font_family(INTER_FAMILY)
            .truncate()
            .text_color(tint(theme, |theme| theme.text.muted))
    })
}
