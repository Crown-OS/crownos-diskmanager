use crownui::kit::{TEXT_SIZE, Theme, use_theme};
use crownui::prelude::{
    CornerRadii, Point, Rect, Signal, Styled, View, canvas, component, hstack, icon, text, vstack,
};

use crate::util::contrast::{GRAPHIC_CONTRAST, legible_against};
use crate::util::tint::tint;

pub const BRAND_LABEL: &str = "Disk Manager";
const DISK_GLYPH: &str = include_str!("../../data/icons/drive-harddisk-symbolic.svg");
const ROW_VERTICAL_PADDING: f32 = 14.0;
const ROW_HORIZONTAL_PADDING: f32 = 8.0;
const LABEL_GAP: f32 = 12.0;
const LABEL_SIZE: f32 = TEXT_SIZE + 2.0;
const TILE_SIZE: f32 = 36.0;
const TILE_RADIUS: f32 = 10.0;
const GLYPH_SIZE: f32 = 22.0;

pub fn brand() -> impl View {
    component(|cx| {
        let theme = use_theme(cx);
        hstack((
            brand_tile(theme),
            text(BRAND_LABEL)
                .text_size(LABEL_SIZE)
                .font_semibold()
                .truncate()
                .text_color(tint(theme, |theme| theme.sidebar.brand_text)),
        ))
        .items_center()
        .gap(LABEL_GAP)
        .py(ROW_VERTICAL_PADDING)
        .px(ROW_HORIZONTAL_PADDING)
        .shrink(0.0)
    })
}

fn brand_tile(theme: Signal<Theme>) -> impl View {
    let accent_fill = canvas(move |runtime, bounds, list| {
        let accent = theme.with(runtime, |theme| {
            let glyph = theme.text.on_accent;
            theme
                .accent
                .map(|stop| legible_against(stop, glyph, GRAPHIC_CONTRAST))
        });
        list.fill(
            Rect::new(Point::ZERO, bounds),
            CornerRadii::uniform(TILE_RADIUS),
            accent.vertical(0.0, bounds.height),
        );
    })
    .absolute()
    .inset(0.0);
    let glyph = icon(DISK_GLYPH, tint(theme, |theme| theme.text.on_accent)).size(GLYPH_SIZE);
    vstack((accent_fill, glyph))
        .size(TILE_SIZE)
        .items_center()
        .justify_center()
        .shrink(0.0)
}
