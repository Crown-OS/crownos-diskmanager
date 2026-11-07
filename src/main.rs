//! # CrownOS Disk Manager
//!
//! * [`crownos_disks`] — the UDisks2 backend: inventory, safety policy and
//!   operations. Knows nothing about UI.
//! * [`state`] — the [`state::Store`] every view is handed, fed by the backend
//!   worker's events.
//! * [`components`] — disk-specific widgets built from CrownUI's kit.
//! * [`pages`] and [`dialogs`] — what the selection shows, and the sheets an
//!   action opens.
//! * [`sidebar`], [`app`] — navigation and the root view.

mod app;
mod components;
mod dialogs;
mod pages;
mod sidebar;
mod state;
mod util;

use anyhow::{Result, bail};
use crownui::AppConfig;
use crownui::kit::{INTER_FAMILY, inter_font};
use crownui::prelude::{BLACK, CornerShape};

const INITIAL_SIZE: (u32, u32) = (1100, 740);
const MIN_SIZE: (u32, u32) = (760, 520);

fn main() -> Result<()> {
    if rustix::process::geteuid().is_root() {
        bail!("Disk Manager runs as your user; it asks for a password when an action needs one.");
    }
    let config = AppConfig::new("Disk Manager")
        .app_id("crownos-diskmanager")
        .size(INITIAL_SIZE.0, INITIAL_SIZE.1)
        .min_size(MIN_SIZE.0, MIN_SIZE.1)
        .clear_color(BLACK)
        .corner_shape(CornerShape::Squircle)
        .font(inter_font().to_vec(), Some(INTER_FAMILY));
    Ok(crownui::run(app::root(), config)?)
}
