use anyhow::{Context, Result};
use capture::Carrier;
use crownos_diskmanager::state::Selection;
use crownos_disks::Event;
use crownos_disks::command::{QuitImpact, Target};
use crownos_disks::guard;
use crownos_disks::journal::{JournalEntry, JournalOperation};
use crownos_disks::model::{ByteSize, Disk, DiskKind, HealthStatus, Inventory};

use super::{background_operation, shot};
use crate::fake_disks::{
    self, SYSTEM_DISK, USB_STICK, WORKSPACE_DISK, disk_named, free_region, health_status, labelled,
    mounted_at, name_where, partition_where,
};
use crate::window::Window;

const GROUP: &str = "Disk pages";
const MAIN: &str = "Main window";
const SCROLL_TO_DETAILS: f32 = 900.0;

pub fn pages(carrier: &mut Carrier) -> Result<()> {
    let inventory = fake_disks::removable();
    {
        let mut window = Window::with(inventory.clone())?;
        shot(
            carrier,
            &mut window,
            MAIN,
            "Overview",
            "Every disk with its usage and health",
        )?;
    }
    {
        let mut window = Window::with(inventory.clone())?;
        window.click_sidebar(SYSTEM_DISK);
        shot(
            carrier,
            &mut window,
            MAIN,
            "System disk",
            "The disk CrownOS runs from, first partition selected",
        )?;
        window.scroll_page(SYSTEM_DISK, SCROLL_TO_DETAILS);
        shot(
            carrier,
            &mut window,
            GROUP,
            "Health and details",
            "The disk page scrolled to its health report and hardware details",
        )?;
    }
    {
        let mut window = Window::with(inventory.clone())?;
        let (disk, block) =
            partition_where(&inventory, SYSTEM_DISK, mounted_at("/")).context("root partition")?;
        window.select(Selection::Partition { disk, block });
        shot(
            carrier,
            &mut window,
            GROUP,
            "Root volume",
            "The mounted system volume selected in the partition map",
        )?;
    }
    {
        let mut window = Window::with(inventory.clone())?;
        window.click_sidebar(WORKSPACE_DISK);
        shot(
            carrier,
            &mut window,
            GROUP,
            "Second NVMe",
            "An internal data disk with one large ext4 volume",
        )?;
    }
    {
        let mut window = Window::with(inventory.clone())?;
        window.click_sidebar(USB_STICK);
        shot(
            carrier,
            &mut window,
            GROUP,
            "USB drive",
            "A removable stick with Eject in the toolbar",
        )?;
        let (disk, block) =
            partition_where(&inventory, USB_STICK, labelled("SANDISK")).context("SANDISK")?;
        window.select(Selection::Partition { disk, block });
        shot(
            carrier,
            &mut window,
            GROUP,
            "USB volume",
            "The stick's exFAT volume, with Open in Files",
        )?;
    }
    {
        let image = name_of(&inventory, |disk| disk.kind == DiskKind::Image)?;
        let mut window = Window::with(inventory.clone())?;
        window.click_sidebar(&image);
        shot(
            carrier,
            &mut window,
            GROUP,
            "Disk image",
            "An opened ISO: no health section, Close Image instead of Eject",
        )?;
    }
    encrypted_pages(carrier)?;
    failing_pages(carrier)?;
    {
        let blank = fake_disks::blank();
        let name = name_of(&blank, |disk| {
            disk.table.is_none() && disk.partitions().next().is_none()
        })?;
        let mut window = Window::with(blank)?;
        window.click_sidebar(&name);
        shot(
            carrier,
            &mut window,
            GROUP,
            "Blank disk",
            "A disk without a partition table: one unallocated region",
        )?;
    }
    {
        let usb = disk_named(&inventory, USB_STICK).context("USB stick")?;
        let mut window = Window::with(inventory.clone())?;
        window.apply(background_operation(
            1,
            "Formatting SANDISK",
            Target::Disk(usb),
            QuitImpact::Harmless,
        ));
        window.click_sidebar(USB_STICK);
        shot(
            carrier,
            &mut window,
            GROUP,
            "Busy disk",
            "Another operation is running, so every action is disabled",
        )?;
    }
    interrupted_page(carrier, &inventory)
}

fn encrypted_pages(carrier: &mut Carrier) -> Result<()> {
    let inventory = fake_disks::encrypted();
    let drive = name_of(&inventory, |disk| {
        disk.partitions()
            .any(|partition| labelled("Vault")(partition))
    })?;
    for (label, name, description) in [
        (
            "Vault",
            "Locked volume",
            "A locked LUKS2 partition offers Unlock",
        ),
        (
            "Photos",
            "Unlocked volume",
            "An unlocked, mounted LUKS2 partition",
        ),
    ] {
        let (disk, block) =
            partition_where(&inventory, &drive, labelled(label)).context("encrypted partition")?;
        let mut window = Window::with(inventory.clone())?;
        window.select(Selection::Partition { disk, block });
        shot(carrier, &mut window, GROUP, name, description)?;
    }
    Ok(())
}

fn failing_pages(carrier: &mut Carrier) -> Result<()> {
    let inventory = fake_disks::failing();
    let roomy = name_of(&inventory, |disk| {
        health_status(disk) != Some(HealthStatus::Failing)
            && free_region(&inventory, &disk.name).is_some()
    })?;
    let (disk, offset, _) = free_region(&inventory, &roomy).context("free space")?;
    {
        let mut window = Window::with(inventory.clone())?;
        window.select(Selection::Free { disk, offset });
        shot(
            carrier,
            &mut window,
            GROUP,
            "Free space",
            "Unpartitioned space selected, offering New Partition",
        )?;
    }
    let failing = name_of(&inventory, |disk| {
        health_status(disk) == Some(HealthStatus::Failing)
    })?;
    let mut window = Window::with(inventory)?;
    window.click_sidebar(&failing);
    shot(
        carrier,
        &mut window,
        GROUP,
        "Failing disk",
        "A hard disk with bad sectors flagged as failing",
    )?;
    window.scroll_page(&failing, SCROLL_TO_DETAILS);
    shot(
        carrier,
        &mut window,
        GROUP,
        "Failing health report",
        "The failing disk's health notes and measurements",
    )?;
    Ok(())
}

fn interrupted_page(carrier: &mut Carrier, inventory: &Inventory) -> Result<()> {
    let disk = disk_named(inventory, SYSTEM_DISK).context("system disk")?;
    let fingerprint =
        guard::fingerprint(inventory, &Target::Disk(disk.clone())).context("fingerprint")?;
    let entry = JournalEntry {
        operation: JournalOperation::Grow {
            target_size: ByteSize(480 * ByteSize::GIB.bytes()),
        },
        disk,
        block: None,
        target_name: "Root".to_owned(),
        fingerprint,
        completed_steps: 1,
        started_at_unix: 1_793_000_000,
    };
    let mut window = Window::with(inventory.clone())?;
    window.apply(Event::Interrupted(vec![entry]));
    window.click_sidebar(SYSTEM_DISK);
    shot(
        carrier,
        &mut window,
        GROUP,
        "Interrupted resize",
        "A resize stopped partway, offering to finish it",
    )
}

fn name_of(inventory: &Inventory, matches: impl Fn(&Disk) -> bool) -> Result<String> {
    name_where(inventory, matches).context("no sample disk matches")
}
