use crownui::kit::Theme;
use crownui::prelude::Color;

use super::contrast::{GRAPHIC_CONTRAST, composite, legible_against};
use super::tint::with_alpha;

const PARTITION_FILL_ALPHA: f32 = 0.3;

fn partition_color(theme: &Theme, index: usize) -> Color {
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

pub fn partition_meter_color(theme: &Theme, index: usize) -> Color {
    legible_against(
        partition_color(theme, index),
        theme.control.track,
        GRAPHIC_CONTRAST,
    )
}

pub fn partition_usage_color(theme: &Theme, index: usize) -> Color {
    let segment_background = composite(partition_fill(theme, index), theme.surface.bg);
    legible_against(
        partition_color(theme, index),
        segment_background,
        GRAPHIC_CONTRAST,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::contrast::contrast_ratio;

    const PALETTE_SIZE: usize = 5;

    #[test]
    fn colours_cycle_through_the_theme() {
        let theme = Theme::light();
        assert_eq!(partition_color(&theme, 0), theme.accent.end);
        assert_eq!(
            partition_color(&theme, PALETTE_SIZE),
            partition_color(&theme, 0)
        );
        assert_ne!(partition_color(&theme, 1), partition_color(&theme, 0));
    }

    #[test]
    fn meter_and_usage_colours_stand_out_in_both_modes() {
        for theme in [Theme::light(), Theme::dark()] {
            for index in 0..PALETTE_SIZE {
                let segment_background = composite(partition_fill(&theme, index), theme.surface.bg);
                assert!(
                    contrast_ratio(partition_meter_color(&theme, index), theme.control.track)
                        >= GRAPHIC_CONTRAST
                );
                assert!(
                    contrast_ratio(partition_usage_color(&theme, index), segment_background)
                        >= GRAPHIC_CONTRAST
                );
            }
        }
    }
}
