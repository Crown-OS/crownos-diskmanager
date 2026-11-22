use anyhow::{Result, bail};
use crownos_diskmanager::{app_config, root};

fn main() -> Result<()> {
    if rustix::process::geteuid().is_root() {
        bail!("Disk Manager runs as your user; it asks for a password when an action needs one.");
    }
    Ok(crownui::run(root(), app_config())?)
}
