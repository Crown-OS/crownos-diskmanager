use crownui::AppConfig;
use crownui::kit::{INTER_FAMILY, inter_font};
use crownui::prelude::{BLACK, CornerShape};

const INITIAL_SIZE: (u32, u32) = (1100, 740);
const MIN_SIZE: (u32, u32) = (760, 520);

/// The window Disk Manager opens in.
pub fn app_config() -> AppConfig {
    AppConfig::new("Disk Manager")
        .app_id("crownos-diskmanager")
        .size(INITIAL_SIZE.0, INITIAL_SIZE.1)
        .min_size(MIN_SIZE.0, MIN_SIZE.1)
        .clear_color(BLACK)
        .corner_shape(CornerShape::Squircle)
        .font(inter_font().to_vec(), Some(INTER_FAMILY))
}
