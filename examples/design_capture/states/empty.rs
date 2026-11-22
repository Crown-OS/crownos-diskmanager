use anyhow::Result;
use capture::Carrier;
use crownos_disks::model::Inventory;
use crownos_disks::{Event, UserError};

use super::shot;
use crate::fake_disks;
use crate::window::Window;

const GROUP: &str = "Empty & error states";

pub fn empty(carrier: &mut Carrier) -> Result<()> {
    {
        let mut window = Window::with(Inventory::default())?;
        shot(
            carrier,
            &mut window,
            GROUP,
            "No disks",
            "Nothing reported yet, or nothing connected",
        )?;
    }
    {
        let mut window = Window::with(fake_disks::system_only())?;
        shot(
            carrier,
            &mut window,
            GROUP,
            "Only the system disk",
            "A hint that plugged-in drives appear here",
        )?;
    }
    let mut window = Window::with(fake_disks::removable())?;
    window.apply(Event::ServiceUnavailable(UserError::new(
        "Storage service unavailable",
        "UDisks2 stopped answering. Start udisks2.service and try again.",
    )));
    shot(
        carrier,
        &mut window,
        GROUP,
        "Service unavailable",
        "UDisks2 is not running",
    )
}
