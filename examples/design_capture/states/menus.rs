use std::time::Duration;

use anyhow::{Context, Result};
use capture::Carrier;
use crownos_diskmanager::state::Selection;
use crowntest::{Role, role};
use crownui::kit::{Toast, icons};

use super::shot;
use crate::fake_disks::{self, SYSTEM_DISK, USB_STICK, labelled, partition_where};
use crate::window::Window;

const GROUP: &str = "Menus & popovers";
const TOOLTIP_DELAY_MS: u64 = 700;
const TOAST_OUTLASTS_CAPTURE: Duration = Duration::from_secs(3600);

pub fn menus(carrier: &mut Carrier) -> Result<()> {
    let inventory = fake_disks::removable();
    let (usb, sandisk) =
        partition_where(&inventory, USB_STICK, labelled("SANDISK")).context("SANDISK")?;
    {
        let mut window = Window::with(inventory.clone())?;
        window.click_sidebar(USB_STICK).click_button("Image");
        shot(
            carrier,
            &mut window,
            GROUP,
            "Image menu",
            "Create or restore a disk image",
        )?;
    }
    {
        let mut window = Window::with(inventory.clone())?;
        window.select(Selection::Partition {
            disk: usb.clone(),
            block: sandisk.clone(),
        });
        window.click_button("More Actions");
        shot(
            carrier,
            &mut window,
            GROUP,
            "More Actions menu",
            "Rename and Delete for the selected volume",
        )?;
    }
    {
        let mut window = Window::with(inventory.clone())?;
        window.right_click(role(Role::MenuItem).name(USB_STICK));
        shot(
            carrier,
            &mut window,
            GROUP,
            "Disk context menu",
            "Right-click on a disk in the sidebar",
        )?;
    }
    {
        let mut window = Window::with(inventory.clone())?;
        window.select(Selection::Partition {
            disk: usb.clone(),
            block: sandisk,
        });
        window.right_click_first(role(Role::Button).text("SANDISK"))?;
        shot(
            carrier,
            &mut window,
            GROUP,
            "Volume context menu",
            "Right-click on a volume in the partition map",
        )?;
    }
    {
        let mut window = Window::with(inventory.clone())?;
        window.click_sidebar(SYSTEM_DISK);
        let erase = window
            .app
            .find(role(Role::Button).name("Erase"))
            .bounds
            .center();
        window.app.pointer_move(erase.x, erase.y);
        window.app.advance_ms(TOOLTIP_DELAY_MS);
        shot(
            carrier,
            &mut window,
            GROUP,
            "Refusal tooltip",
            "Why Erase is disabled on the system disk",
        )?;
    }
    let mut window = Window::with(inventory)?;
    window.show_toast(
        Toast::new("Safe to remove SanDisk Ultra")
            .icon(icons::CIRCLE_CHECK)
            .duration(TOAST_OUTLASTS_CAPTURE),
    );
    shot(
        carrier,
        &mut window,
        GROUP,
        "Done toast",
        "A finished operation confirms itself in a toast",
    )
}
