use crownui::prelude::{BLACK, Color, WHITE};

pub const GRAPHIC_CONTRAST: f32 = 3.0;
const ADJUST_STEPS: u8 = 20;

fn linear_channel(channel: f32) -> f32 {
    if channel <= 0.039_28 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

fn relative_luminance(color: Color) -> f32 {
    0.2126 * linear_channel(color.r)
        + 0.7152 * linear_channel(color.g)
        + 0.0722 * linear_channel(color.b)
}

pub fn contrast_ratio(first: Color, second: Color) -> f32 {
    let (first, second) = (relative_luminance(first), relative_luminance(second));
    (first.max(second) + 0.05) / (first.min(second) + 0.05)
}

pub fn mix(from: Color, to: Color, progress: f32) -> Color {
    let blend = |start: f32, end: f32| start + (end - start) * progress;
    Color {
        r: blend(from.r, to.r),
        g: blend(from.g, to.g),
        b: blend(from.b, to.b),
        a: blend(from.a, to.a),
    }
}

pub fn composite(top: Color, bottom: Color) -> Color {
    Color {
        a: 1.0,
        ..mix(Color { a: 1.0, ..bottom }, Color { a: 1.0, ..top }, top.a)
    }
}

pub fn legible_against(color: Color, background: Color, min_ratio: f32) -> Color {
    let extreme = if contrast_ratio(WHITE, background) > contrast_ratio(BLACK, background) {
        WHITE
    } else {
        BLACK
    };
    (0..=ADJUST_STEPS)
        .map(|step| mix(color, extreme, f32::from(step) / f32::from(ADJUST_STEPS)))
        .find(|candidate| contrast_ratio(*candidate, background) >= min_ratio)
        .unwrap_or(extreme)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crownui::prelude::rgb;

    const TEXT_CONTRAST: f32 = 4.5;

    #[test]
    fn black_on_white_is_twenty_one_to_one() {
        assert!((contrast_ratio(BLACK, WHITE) - 21.0).abs() < 0.01);
    }

    #[test]
    fn legible_colours_reach_the_requested_ratio() {
        let background = rgb(0xf3f3f3);
        let adjusted = legible_against(rgb(0x4ade80), background, GRAPHIC_CONTRAST);
        assert!(contrast_ratio(adjusted, background) >= GRAPHIC_CONTRAST);
    }

    #[test]
    fn colours_that_already_pass_are_untouched() {
        let colour = rgb(0x17151f);
        assert_eq!(legible_against(colour, WHITE, TEXT_CONTRAST), colour);
    }

    #[test]
    fn compositing_a_translucent_colour_lands_between_both() {
        let blended = composite(Color { a: 0.5, ..WHITE }, BLACK);
        assert!((blended.r - 0.5).abs() < 0.001);
        assert!((blended.a - 1.0).abs() < f32::EPSILON);
    }
}
