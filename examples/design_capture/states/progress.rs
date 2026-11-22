use anyhow::{Context, Result};
use capture::Carrier;
use crownos_disks::command::{OperationId, Progress, QuitImpact, Target};
use crownos_disks::{Event, UserError};

use super::{background_operation, shot};
use crate::fake_disks::{self, USB_STICK, disk_named};
use crate::window::Window;

const GROUP: &str = "Progress & results";
const RESTORE: OperationId = OperationId(3);

fn restoring(progress: Progress, phase: &str) -> Result<Window> {
    let inventory = fake_disks::removable();
    let usb = disk_named(&inventory, USB_STICK).context("USB stick")?;
    let mut window = Window::with(inventory)?;
    window.click_sidebar(USB_STICK);
    window.apply(Event::OperationStarted {
        id: RESTORE,
        label: "Restoring crownos-2026.11-x86_64.iso".to_owned(),
        target: Target::Disk(usb),
        cancelable: true,
        quit_impact: QuitImpact::LeavesTargetUnusable,
    });
    window.apply(Event::OperationProgress {
        id: RESTORE,
        phase: phase.to_owned(),
        progress,
    });
    Ok(window)
}

pub fn progress(carrier: &mut Carrier) -> Result<()> {
    let writing = Progress {
        fraction: Some(0.42),
        bytes_per_second: Some(45_200_000),
        seconds_remaining: Some(170),
    };
    {
        let mut window = restoring(writing, "Writing")?;
        shot(
            carrier,
            &mut window,
            GROUP,
            "Progress",
            "A long operation with its phase, rate and time left",
        )?;
        window.click_button("Cancel");
        shot(
            carrier,
            &mut window,
            GROUP,
            "Cancelling",
            "Cancel pressed, waiting for the copy to stop",
        )?;
    }
    {
        let mut window = restoring(Progress::default(), "Preparing")?;
        shot(
            carrier,
            &mut window,
            GROUP,
            "Preparing",
            "An operation without a fraction yet shows a spinner",
        )?;
    }
    let inventory = fake_disks::removable();
    let usb = disk_named(&inventory, USB_STICK).context("USB stick")?;
    let mut window = Window::with(inventory)?;
    window.apply(background_operation(
        9,
        "Formatting SANDISK",
        Target::Disk(usb),
        QuitImpact::Harmless,
    ));
    let error = UserError::new(
        "Couldn't format SANDISK",
        "SANDISK is in use by another app. Close the files on it and try again.",
    )
    .with_details("mkfs.exfat: /dev/sdb1: Device or resource busy");
    window.apply(Event::OperationFinished {
        id: OperationId(9),
        result: Err(error),
    });
    shot(
        carrier,
        &mut window,
        GROUP,
        "Error",
        "A failed operation explains itself",
    )?;
    window.click_button("Show details");
    shot(
        carrier,
        &mut window,
        GROUP,
        "Error details",
        "The raw error behind the disclosure",
    )
}
