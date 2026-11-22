use anyhow::{Context, Result};
use capture::Carrier;
use crownos_diskmanager::state::{DialogRequest, Selection};
use crownos_disks::command::{QuitImpact, Target};

use super::{background_operation, shot};
use crate::fake_disks::{
    self, DATA_VOLUME, USB_STICK, WORKSPACE_DISK, disk_named, labelled, partition_where,
    partition_where_any,
};
use crate::window::Window;

const GROUP: &str = "Dialogs";

pub fn dialogs(carrier: &mut Carrier) -> Result<()> {
    let inventory = fake_disks::removable();
    let usb = disk_named(&inventory, USB_STICK).context("USB stick")?;
    let (_, sandisk) =
        partition_where(&inventory, USB_STICK, labelled("SANDISK")).context("SANDISK")?;
    {
        let mut window = Window::with(inventory.clone())?;
        window
            .click_sidebar(USB_STICK)
            .open(DialogRequest::EraseDisk(usb.clone()));
        shot(
            carrier,
            &mut window,
            GROUP,
            "Erase disk",
            "Name, format and partition scheme for the erased disk",
        )?;
        window.click_button("Erase…");
        shot(
            carrier,
            &mut window,
            GROUP,
            "Erase confirmation",
            "A destructive confirmation listing what is lost",
        )?;
    }
    let workspace = disk_named(&inventory, WORKSPACE_DISK).context("workspace disk")?;
    {
        let mut window = Window::with(inventory.clone())?;
        window
            .click_sidebar(WORKSPACE_DISK)
            .open(DialogRequest::EraseDisk(workspace.clone()))
            .click_button("Erase…");
        shot(
            carrier,
            &mut window,
            GROUP,
            "Erase internal disk",
            "An internal disk asks for its name to be typed",
        )?;
        window.type_into("Name to confirm", WORKSPACE_DISK);
        shot(
            carrier,
            &mut window,
            GROUP,
            "Erase internal disk typed",
            "The typed name unlocks the destructive button",
        )?;
    }
    for (menu, item, name, description) in [
        (
            "More Actions",
            "Delete",
            "Delete partition",
            "Deleting a volume lists what is lost",
        ),
        (
            "Image",
            "Create Image",
            "Create image",
            "Compression and destination for a disk image",
        ),
        (
            "Image",
            "Restore Image",
            "Restore image",
            "Choose an image to write over the volume",
        ),
    ] {
        let mut window = Window::with(inventory.clone())?;
        window.select(Selection::Partition {
            disk: usb.clone(),
            block: sandisk.clone(),
        });
        window.click_button(menu).click_menu_item(item);
        shot(carrier, &mut window, GROUP, name, description)?;
    }
    {
        let failing = fake_disks::failing();
        let (disk, data) = partition_where_any(&failing, labelled(DATA_VOLUME)).context("Data")?;
        let mut window = Window::with(failing)?;
        window.select(Selection::Partition { disk, block: data });
        window.click_button("Check & Repair");
        shot(
            carrier,
            &mut window,
            GROUP,
            "Check & Repair",
            "A simple confirmation before checking a volume",
        )?;
    }
    {
        let mut window = Window::with(inventory.clone())?;
        window.click_sidebar(USB_STICK);
        window.open(DialogRequest::Format(sandisk.clone()));
        let unplugged = {
            let mut inventory = inventory.clone();
            inventory.disks.retain(|disk| disk.name != USB_STICK);
            inventory
        };
        window.set_inventory(unplugged);
        shot(
            carrier,
            &mut window,
            GROUP,
            "Disk disconnected",
            "The open sheet's disk was unplugged",
        )?;
    }
    let mut window = Window::with(inventory)?;
    window.apply(background_operation(
        4,
        "Creating image of SanDisk Ultra",
        Target::Disk(usb),
        QuitImpact::LosesImage,
    ));
    window.apply(background_operation(
        5,
        "Checking Workspace",
        Target::Disk(workspace),
        QuitImpact::FinishesInBackground,
    ));
    window.app.request_close();
    window.app.settle();
    shot(
        carrier,
        &mut window,
        GROUP,
        "Quit while busy",
        "Closing the window while operations are still running",
    )
}
