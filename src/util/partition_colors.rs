use crownui::kit::Theme;
use crownui::prelude::Color;

use super::tint::with_alpha;

const PARTITION_FILL_ALPHA: f32 = 0.3;

pub fn partition_color(theme: &Theme, index: usize) -> Color {
    let palette = [
        theme.accent.end,
        theme.status.success,
        theme.status.warning,
        theme.accent.start,
        theme.status.neutral,
    ];
    palette[index % palette.len()]
}

pub fn partition_fill(theme: &Theme, index: usize) -> Color {
    with_alpha(partition_color(theme, index), PARTITION_FILL_ALPHA)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_cycle_through_the_theme() {
        let theme = Theme::light();
        assert_eq!(partition_color(&theme, 0), theme.accent.end);
        assert_eq!(partition_color(&theme, 5), partition_color(&theme, 0));
        assert_ne!(partition_color(&theme, 1), partition_color(&theme, 0));
    }
}
