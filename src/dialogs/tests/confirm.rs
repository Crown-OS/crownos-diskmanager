use crownos_disks::Command;
use crownos_disks::command::PartitionScheme;
use crownos_disks::fixtures::{blank_disk, removable};
use crownos_disks::model::FilesystemKind;
use crowntest::{Role, role};

use super::harness::Harness;
use super::{block_titled, disk_named};
use crate::state::DialogRequest;

const STICK: &str = "SanDisk Ultra";
const BLANK: &str = "WDC WD20EZRZ-00Z5HB0";

fn erase(inventory: crownos_disks::model::Inventory, name: &str) -> Harness {
    let disk = disk_named(&inventory, name);
    let mut harness = Harness::opened(inventory, DialogRequest::EraseDisk(disk));
    harness.click("Erase…");
    harness
}

#[test]
fn the_destructive_button_names_the_action_and_target() {
    let mut harness = erase(removable(), STICK);
    assert!(
        harness
            .app
            .exists(role(Role::Button).name("Erase SanDisk Ultra"))
    );
    assert!(!harness.app.exists(role(Role::Button).name("OK")));
    assert!(harness.has_text("Everything on SanDisk Ultra will be permanently erased."));
}

#[test]
fn everything_that_will_be_lost_is_listed() {
    let mut harness = erase(removable(), STICK);
    assert!(
        harness.has_text("• SANDISK · exFAT · 12.0 GB used"),
        "{}",
        harness.app.semantics_snapshot()
    );
}

#[test]
fn destructive_confirmations_start_on_cancel_and_ignore_enter() {
    let mut harness = erase(removable(), STICK);
    assert_eq!(harness.focused_name().as_deref(), Some("Cancel"));
    harness
        .app
        .focus(role(Role::Button).name("Erase SanDisk Ultra"));
    harness.app.press_key("Enter").settle();
    assert!(harness.sent().is_empty());
    assert_eq!(harness.dialogs(), 1);
}

#[test]
fn confirming_sends_the_erase_with_the_captured_fingerprint() {
    let inventory = removable();
    let disk = disk_named(&inventory, STICK);
    let expected = crownos_disks::guard::fingerprint(
        &inventory,
        &crownos_disks::command::Target::Disk(disk.clone()),
    );
    let mut harness = erase(inventory, STICK);
    harness.click("Erase SanDisk Ultra");
    let sent = harness.sent();
    let [
        Command::EraseDisk {
            disk: sent_disk,
            scheme,
            spec,
            expected: sent_expected,
        },
    ] = sent.as_slice()
    else {
        panic!("expected one erase, got {sent:?}");
    };
    assert_eq!(sent_disk, &disk);
    assert_eq!(*scheme, PartitionScheme::Gpt);
    assert_eq!(spec.filesystem, FilesystemKind::Ext4);
    assert!(spec.encryption.is_none());
    assert_eq!(Some(sent_expected), expected.as_ref());
    assert_eq!(harness.request(), None);
    assert_eq!(harness.dialogs(), 0);
}

#[test]
fn internal_disks_need_their_name_typed_exactly() {
    let mut harness = erase(blank_disk(), BLANK);
    let action = format!("Erase {BLANK}");
    assert!(harness.button_disabled(&action));
    harness
        .app
        .click(role(Role::TextField).name("Name to confirm"));
    harness.app.type_text("WDC").settle();
    assert!(harness.button_disabled(&action));
    harness.app.type_text(&BLANK["WDC".len()..]).settle();
    assert!(!harness.button_disabled(&action));
    harness.click(&action);
    assert!(matches!(
        harness.sent().as_slice(),
        [Command::EraseDisk { .. }]
    ));
}

#[test]
fn delete_goes_straight_to_a_destructive_confirmation() {
    let inventory = removable();
    let block = block_titled(&inventory, "SANDISK");
    let mut harness = Harness::opened(inventory, DialogRequest::Delete(block.clone()));
    assert!(
        harness
            .app
            .exists(role(Role::Button).name("Delete SANDISK"))
    );
    assert!(harness.has_text("• SANDISK · exFAT · 12.0 GB used"));
    harness.click("Delete SANDISK");
    let sent = harness.sent();
    assert!(
        matches!(sent.as_slice(), [Command::DeletePartition { block: sent, .. }] if *sent == block),
        "{sent:?}"
    );
}

#[test]
fn cancel_closes_the_whole_request() {
    let mut harness = erase(removable(), STICK);
    harness.click("Cancel");
    assert_eq!(harness.request(), None);
    assert_eq!(harness.dialogs(), 0);
    assert!(harness.sent().is_empty());
}

#[test]
fn unplugging_the_target_closes_the_sheet_with_a_note() {
    let inventory = removable();
    let disk = disk_named(&inventory, STICK);
    let mut harness = Harness::opened(inventory, DialogRequest::EraseDisk(disk));
    harness.set_inventory(crownos_disks::fixtures::desktop());
    assert!(harness.has_text("SanDisk Ultra is no longer connected"));
    harness.click("OK");
    assert_eq!(harness.request(), None);
}
