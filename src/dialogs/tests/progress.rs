use crownos_disks::command::{OperationId, Progress, Target};
use crownos_disks::fixtures::removable;
use crownos_disks::{Command, Event};
use crowntest::{Role, role};

use super::disk_named;
use super::harness::Harness;

const ID: OperationId = OperationId(3);

fn running(cancelable: bool) -> Harness {
    let inventory = removable();
    let disk = disk_named(&inventory, "SanDisk Ultra");
    let mut harness = Harness::new(inventory);
    harness.apply(Event::OperationStarted {
        id: ID,
        label: "Restoring ubuntu.iso".into(),
        target: Target::Disk(disk),
        cancelable,
        quit_impact: crownos_disks::command::QuitImpact::Harmless,
    });
    let progress = Progress {
        fraction: Some(0.25),
        bytes_per_second: Some(45_200_000),
        seconds_remaining: Some(170),
    };
    harness.apply(Event::OperationProgress {
        id: ID,
        phase: "Writing".into(),
        progress,
    });
    harness
}

#[test]
fn long_operations_show_label_phase_progress_rate_and_time() {
    let mut harness = running(true);
    assert_eq!(harness.dialogs(), 1);
    assert!(harness.has_text("Restoring ubuntu.iso"));
    assert!(harness.has_text("Writing"));
    assert!(harness.has_text("45.2 MB/s · About 3 minutes left"));
    assert!(harness.app.exists(role(Role::ProgressBar)));
}

#[test]
fn cancel_is_offered_only_when_the_operation_can_stop() {
    let mut harness = running(false);
    assert!(!harness.app.exists(role(Role::Button).name("Cancel")));

    let mut harness = running(true);
    harness.click("Cancel");
    assert!(matches!(harness.sent().as_slice(), [Command::Cancel(ID)]));
    assert!(harness.button_disabled("Cancelling…"));
}

#[test]
fn escape_does_not_hide_a_running_operation() {
    let mut harness = running(true);
    harness
        .app
        .focus(role(Role::Group).name("Operation in progress"));
    harness.app.press_key("Escape").settle();
    assert_eq!(harness.dialogs(), 1);
}

#[test]
fn the_sheet_leaves_when_the_operation_finishes() {
    let mut harness = running(true);
    let completion = crownos_disks::command::Completion {
        message: "Ready to boot".into(),
    };
    harness.apply(Event::OperationFinished {
        id: ID,
        result: Ok(completion),
    });
    assert_eq!(harness.dialogs(), 0);
}

#[test]
fn quick_operations_get_no_sheet() {
    let inventory = removable();
    let disk = disk_named(&inventory, "SanDisk Ultra");
    let mut harness = Harness::new(inventory);
    harness.apply(Event::OperationStarted {
        id: ID,
        label: "Mounting".into(),
        target: Target::Disk(disk),
        cancelable: false,
        quit_impact: crownos_disks::command::QuitImpact::Harmless,
    });
    assert_eq!(harness.dialogs(), 0);
}
