use crownui::kit::Theme;
use crownui::prelude::{Color, IntoProp, Signal, bind};

pub fn tint(
    theme: Signal<Theme>,
    pick: impl Fn(&Theme) -> Color + 'static,
) -> impl IntoProp<Color> {
    bind(move |runtime| theme.with(runtime, &pick))
}

pub fn with_alpha(color: Color, alpha: f32) -> Color {
    Color {
        a: color.a * alpha,
        ..color
    }
}
