//! # CrownOS Disk Manager
//!
//! * [`crownos_disks`] — the UDisks2 backend: inventory, safety policy and
//!   operations. Knows nothing about UI.
//! * [`state`] — the [`state::Store`] every view is handed, fed by the backend
//!   worker's events.
//! * `components` — disk-specific widgets built from CrownUI's kit.
//! * `pages` and `dialogs` — what the selection shows, and the sheets an
//!   action opens.
//! * `sidebar`, `app` — navigation and the root view.

mod app;
mod components;
mod dialogs;
mod pages;
mod sidebar;
pub mod state;
mod util;

pub use app::{app_config, root, shell};
