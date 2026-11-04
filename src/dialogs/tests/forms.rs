use crownos_disks::Command;
use crownos_disks::UserError;
use crownos_disks::command::{Compression, OperationId, Target};
use crownos_disks::fixtures::{encrypted, removable};
use crownos_disks::{Event, model::FilesystemKind};
use crowntest::{Role, role};

use super::harness::Harness;
use super::{block_titled, disk_named};
use crate::state::DialogRequest;

const PASSWORD: &str = "Correct-Horse-42";

fn type_into(harness: &mut Harness, field: &str, text: &str) {
    harness.app.click(role(Role::TextField).name(field));
    harness.app.type_text(text).settle();
}

#[test]
fn rename_waits_for_a_changed_name_and_submits_on_enter() {
    let inventory = removable();
    let block = block_titled(&inventory, "SANDISK");
    let mut harness = Harness::opened(inventory, DialogRequest::Rename(block.clone()));
    assert!(harness.button_disabled("Rename"));
    type_into(&mut harness, "Name", "2");
    assert!(!harness.button_disabled("Rename"));
    harness.app.press_key("Enter").settle();
    let sent = harness.sent();
    assert!(
        matches!(sent.as_slice(), [Command::Rename { block: sent, label }] if *sent == block && label == "SANDISK2"),
        "{sent:?}"
    );
    assert_eq!(harness.request(), None);
}

#[test]
fn an_invalid_name_is_explained_and_blocks_renaming() {
    let inventory = removable();
    let block = block_titled(&inventory, "SANDISK");
    let mut harness = Harness::opened(inventory, DialogRequest::Rename(block));
    type_into(&mut harness, "Name", "-way-too-long-for-exfat");
    assert!(harness.button_disabled("Rename"));
    assert!(
        harness.has_text("at most"),
        "{}",
        harness.app.semantics_snapshot()
    );
}

fn unlock_vault() -> (Harness, crownos_disks::model::BlockId) {
    let inventory = encrypted();
    let block = block_titled(&inventory, "Vault");
    let mut harness = Harness::opened(inventory, DialogRequest::Unlock(block.clone()));
    harness.type_secret("Password", PASSWORD);
    harness.app.press_key("Enter").settle();
    (harness, block)
}

#[test]
fn unlock_submits_on_enter_and_clears_the_field() {
    let (mut harness, block) = unlock_vault();
    let sent = harness.sent();
    let [
        Command::Unlock {
            block: sent_block,
            passphrase,
        },
    ] = sent.as_slice()
    else {
        panic!("expected one unlock, got {sent:?}");
    };
    assert_eq!(sent_block, &block);
    assert_eq!(passphrase.expose(), PASSWORD);
    assert!(harness.has_text("Unlocking…"));
    assert!(harness.button_disabled("Unlock"));
}

#[test]
fn a_wrong_password_is_shown_inside_the_unlock_sheet() {
    let (mut harness, block) = unlock_vault();
    let id = OperationId(7);
    harness.apply(Event::OperationStarted {
        id,
        label: "Unlocking Vault".into(),
        target: Target::Block(block),
        cancelable: false,
        quit_impact: crownos_disks::command::QuitImpact::Harmless,
    });
    let error = UserError::new("Wrong password", "The password for Vault is incorrect.");
    harness.apply(Event::OperationFinished {
        id,
        result: Err(error),
    });
    assert_eq!(harness.dialogs(), 1);
    assert!(harness.has_text("The password for Vault is incorrect."));
    assert!(matches!(harness.request(), Some(DialogRequest::Unlock(_))));
}

#[test]
fn a_successful_unlock_closes_the_sheet() {
    let (mut harness, block) = unlock_vault();
    let id = OperationId(8);
    harness.apply(Event::OperationStarted {
        id,
        label: "Unlocking Vault".into(),
        target: Target::Block(block),
        cancelable: false,
        quit_impact: crownos_disks::command::QuitImpact::Harmless,
    });
    let completion = crownos_disks::command::Completion {
        message: "Vault is unlocked".into(),
    };
    harness.apply(Event::OperationFinished {
        id,
        result: Ok(completion),
    });
    assert_eq!(harness.request(), None);
    assert_eq!(harness.dialogs(), 0);
}

#[test]
fn resizing_a_locked_volume_offers_to_unlock_first() {
    let inventory = encrypted();
    let block = block_titled(&inventory, "Vault");
    let mut harness = Harness::opened(inventory, DialogRequest::Resize(block.clone()));
    assert!(harness.has_text("Unlock first"));
    harness.click("Unlock…");
    assert_eq!(harness.request(), Some(DialogRequest::Unlock(block)));
    assert!(harness.app.exists(role(Role::TextField).name("Password")));
}

#[test]
fn resizing_an_unlocked_luks2_volume_asks_for_its_password() {
    let inventory = encrypted();
    let block = block_titled(&inventory, "Photos");
    let mut harness = Harness::opened(inventory, DialogRequest::Resize(block));
    assert!(
        harness
            .app
            .exists(role(Role::TextField).name("Encryption password")),
        "{}",
        harness.app.semantics_snapshot()
    );
    assert!(harness.button_disabled("Resize"));
}

#[test]
fn new_partition_offers_the_free_space_and_creates_it() {
    let inventory = encrypted();
    let disk = disk_named(&inventory, "Samsung PSSD T7");
    let (offset, size) = inventory
        .disk(&disk)
        .and_then(|disk| {
            disk.layout.iter().find_map(|region| match region {
                crownos_disks::model::Region::Free { offset, size } => Some((*offset, *size)),
                crownos_disks::model::Region::Partition(_) => None,
            })
        })
        .expect("the fixture has free space");
    let mut harness = Harness::opened(
        inventory,
        DialogRequest::NewPartition {
            disk: disk.clone(),
            offset,
            size,
        },
    );
    assert!(harness.has_text("Between 1.0 MB and 33.8 GB"));
    harness.click("Create…");
    assert!(
        harness
            .app
            .exists(role(Role::Button).name("Create Partition"))
    );
    harness.click("Create Partition");
    let sent = harness.sent();
    let [
        Command::CreatePartition {
            disk: sent_disk,
            offset: sent_offset,
            size: sent_size,
            spec,
            ..
        },
    ] = sent.as_slice()
    else {
        panic!("expected one partition, got {sent:?}");
    };
    assert_eq!((sent_disk, *sent_offset), (&disk, offset));
    assert!(
        *sent_size <= size && sent_size.bytes() % crownos_disks::model::ByteSize::MIB.bytes() == 0
    );
    assert_eq!(spec.filesystem, FilesystemKind::Ext4);
}

#[test]
fn encryption_needs_matching_passwords_and_warns_in_the_confirmation() {
    let inventory = removable();
    let block = block_titled(&inventory, "SANDISK");
    let mut harness = Harness::opened(inventory, DialogRequest::Format(block));
    harness
        .app
        .click(role(Role::Switch).name("Encrypt with password"));
    harness.app.settle();
    harness.type_secret("Password", PASSWORD);
    harness.type_secret("Repeat password", "nope");
    assert!(harness.button_disabled("Format…"));
    assert!(harness.has_text("Passwords don't match"));
    assert!(harness.has_text("Strong password"));
    harness
        .app
        .click(role(Role::TextField).name("Repeat password"));
    for _ in 0.."nope".len() {
        harness.app.press_key("Backspace");
    }
    harness.type_secret("Repeat password", PASSWORD);
    assert!(!harness.button_disabled("Format…"));
    harness.click("Format…");
    assert!(harness.has_text("If you forget this password, the data can't be recovered."));
    harness.click("Format SANDISK");
    let sent = harness.sent();
    let [Command::Format { spec, .. }] = sent.as_slice() else {
        panic!("expected one format, got {sent:?}");
    };
    assert_eq!(
        spec.encryption.as_ref().map(|secret| secret.expose()),
        Some(PASSWORD)
    );
}

#[test]
fn unavailable_formats_name_the_missing_tool() {
    let inventory = removable();
    let block = block_titled(&inventory, "SANDISK");
    let mut harness = Harness::opened(inventory, DialogRequest::Format(block));
    assert!(harness.has_text("Requires the exfatprogs package."));
    harness.click("More formats");
    assert!(harness.app.exists(role(Role::RadioButton).text("Swap")));
}

#[test]
fn create_image_waits_for_a_destination() {
    let inventory = removable();
    let disk = disk_named(&inventory, "SanDisk Ultra");
    let mut harness = Harness::opened(inventory, DialogRequest::CreateImage(Target::Disk(disk)));
    assert!(harness.has_text("SanDisk Ultra · 61.5 GB"));
    assert!(harness.has_text("Recommended: much smaller and still fast."));
    assert!(harness.button_disabled("Create Image"));
    assert!(harness.has_text("No file chosen"));
    let _ = Compression::Zstd;
}

#[test]
fn restore_image_waits_for_an_image_and_verifies_by_default() {
    let inventory = removable();
    let disk = disk_named(&inventory, "SanDisk Ultra");
    let mut harness = Harness::opened(inventory, DialogRequest::RestoreImage(Target::Disk(disk)));
    assert!(harness.button_disabled("Restore…"));
    assert!(harness.has_text("No image chosen"));
    assert_eq!(
        harness
            .app
            .find(role(Role::Switch).name("Verify after writing"))
            .states
            .checked,
        Some(true)
    );
}
