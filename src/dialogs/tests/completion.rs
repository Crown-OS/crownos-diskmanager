use crownos_disks::command::{Completion, OperationId, Target};
use crownos_disks::fixtures::removable;
use crownos_disks::model::BlockId;
use crownos_disks::{Command, Event, UserError};

use super::block_titled;
use super::harness::Harness;
use crate::state::DialogRequest;

const ID: OperationId = OperationId(11);

fn start(harness: &mut Harness) {
    let target = Target::Block(BlockId::new("/org/freedesktop/UDisks2/block_devices/sda1"));
    harness.apply(Event::OperationStarted {
        id: ID,
        label: "Working".into(),
        target,
        cancelable: false,
        quit_impact: crownos_disks::command::QuitImpact::Harmless,
    });
}

fn finish(harness: &mut Harness, result: Result<Completion, UserError>) {
    start(harness);
    harness.apply(Event::OperationFinished { id: ID, result });
}

#[test]
fn errors_open_a_sheet_with_details_behind_a_disclosure() {
    let mut harness = Harness::new(removable());
    let error = UserError::new("Couldn't format SANDISK", "SANDISK is in use.")
        .with_details("mkfs.exfat: device busy");
    finish(&mut harness, Err(error));
    assert_eq!(harness.dialogs(), 1);
    assert!(harness.has_text("SANDISK is in use."));
    assert!(!harness.has_text("mkfs.exfat: device busy"));
    harness.click("Show details");
    assert!(harness.has_text("mkfs.exfat: device busy"));
    harness.click("Hide details");
    assert!(!harness.has_text("mkfs.exfat: device busy"));
    harness.click("OK");
    assert_eq!(harness.dialogs(), 0);
}

#[test]
fn silent_errors_show_nothing() {
    let mut harness = Harness::new(removable());
    let error = UserError {
        silent: true,
        ..UserError::new("Cancelled", "The password prompt was dismissed.")
    };
    finish(&mut harness, Err(error));
    assert_eq!(harness.dialogs(), 0);
    assert!(!harness.has_text("Cancelled"));
}

#[test]
fn plain_results_become_toasts() {
    let mut harness = Harness::new(removable());
    start(&mut harness);
    let completion = Completion {
        message: "Safe to remove SanDisk Ultra".into(),
    };
    harness.apply_briefly(Event::OperationFinished {
        id: ID,
        result: Ok(completion),
    });
    assert_eq!(harness.dialogs(), 0);
    assert!(harness.has_text("Safe to remove SanDisk Ultra"));
}

#[test]
fn repair_is_a_simple_confirmation() {
    let inventory = removable();
    let block = block_titled(&inventory, "SANDISK");
    let mut harness = Harness::opened(inventory, DialogRequest::Repair(block.clone()));
    assert!(harness.has_text("Check & Repair SANDISK?"));
    assert!(harness.has_text("SANDISK will be unmounted while it's checked and repaired."));
    harness.click("Check & Repair");
    assert!(matches!(harness.sent().as_slice(), [Command::Repair(sent)] if *sent == block));
}
