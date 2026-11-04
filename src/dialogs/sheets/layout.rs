use crownui::kit::{ButtonOptions, ButtonVariant, DialogOptions, button, dialog, use_theme};
use crownui::prelude::{
    IntoProp, Listeners, Signal, Styled, View, ViewSeq, bind, component, hstack, spacer, text,
    vstack,
};

const TITLE_SIZE: f32 = 15.0;
const CAPTION_SIZE: f32 = 12.0;
const ACTION_GAP: f32 = 8.0;
const ACTION_TOP_PADDING: f32 = 8.0;
const FIELD_GAP: f32 = 6.0;
const SECTION_GAP: f32 = 14.0;

pub fn sheet_title(title: impl IntoProp<String>) -> impl View {
    text(title).text_size(TITLE_SIZE).font_semibold()
}

pub fn muted_text(content: impl IntoProp<String> + 'static) -> impl View {
    component(move |cx| {
        let theme = use_theme(cx);
        text(content).text_color(bind(move |runtime| {
            theme.with(runtime, |theme| theme.text.muted)
        }))
    })
}

pub fn caption(content: impl IntoProp<String> + 'static) -> impl View {
    component(move |cx| {
        let theme = use_theme(cx);
        text(content)
            .text_size(CAPTION_SIZE)
            .font_semibold()
            .text_color(bind(move |runtime| {
                theme.with(runtime, |theme| theme.text.muted)
            }))
    })
}

/// A caption above its control.
pub fn labeled(title: &'static str, control: impl View) -> impl View {
    vstack((caption(title), control)).gap(FIELD_GAP)
}

/// A column of form sections with even spacing.
pub fn form<C: ViewSeq>(sections: C) -> impl View {
    vstack(sections).gap(SECTION_GAP)
}

/// The trailing row of buttons; the last one is the sheet's main action.
pub fn actions<C: ViewSeq>(buttons: C) -> impl View {
    hstack((spacer(), hstack(buttons).gap(ACTION_GAP)))
        .pt(ACTION_TOP_PADDING)
        .items_center()
}

pub fn cancel_button(open: Signal<bool>) -> impl View {
    button("Cancel")
        .variant(ButtonVariant::Secondary)
        .on_click(move |cx, _| open.set(cx, false))
}

/// A form sheet: it keeps what was typed when the scrim is pressed by accident.
pub fn form_dialog<C: ViewSeq>(
    open: Signal<bool>,
    content: impl FnMut() -> C + 'static,
) -> impl View {
    dialog(open, content).dismiss_on_scrim(false)
}
