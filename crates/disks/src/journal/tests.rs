use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use super::*;
use crate::guard::test_inventory;

struct ScratchDirectory(PathBuf);

impl ScratchDirectory {
    fn new() -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let unique = format!(
            "crownos-journal-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        Self(std::env::temp_dir().join(unique))
    }
}

impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn entry(started_at_unix: u64) -> JournalEntry {
    let disk = test_inventory::usb_disk();
    JournalEntry {
        operation: JournalOperation::Shrink {
            target_size: ByteSize(1 << 30),
        },
        disk: disk.id.clone(),
        block: Some(BlockId::new(test_inventory::PHOTOS_BLOCK)),
        target_name: "Photos".to_owned(),
        fingerprint: crate::guard::disk_fingerprint(&disk),
        completed_steps: 0,
        started_at_unix,
    }
}

fn mode(path: &Path) -> u32 {
    std::fs::metadata(path)
        .map(|metadata| metadata.permissions().mode() & 0o777)
        .unwrap_or_default()
}

#[test]
fn entries_survive_until_finished() {
    let scratch = ScratchDirectory::new();
    let journal = Journal::at(&scratch.0);
    assert_eq!(journal.pending().unwrap(), []);

    let mut running = entry(1_700_000_000);
    journal.begin(&running).unwrap();
    journal.advance(&mut running).unwrap();
    journal.advance(&mut running).unwrap();
    assert_eq!(running.completed_steps, 2);
    assert_eq!(journal.pending().unwrap(), [running.clone()]);

    let older = entry(1_600_000_000);
    journal.begin(&older).unwrap();
    assert_eq!(journal.pending().unwrap(), [older.clone(), running.clone()]);

    journal.finish(&running).unwrap();
    journal.finish(&running).unwrap();
    assert_eq!(journal.pending().unwrap(), [older]);
}

#[test]
fn files_are_private_and_leave_no_temporaries() {
    let scratch = ScratchDirectory::new();
    let journal = Journal::at(&scratch.0);
    journal.begin(&entry(1)).unwrap();
    let directory = scratch.0.join("operations");
    assert_eq!(mode(&directory), 0o700);
    let files: Vec<_> = std::fs::read_dir(&directory)
        .unwrap()
        .map(|item| item.unwrap().path())
        .collect();
    assert_eq!(files.len(), 1);
    assert_eq!(mode(&files[0]), 0o600);
    assert_eq!(
        files[0]
            .extension()
            .and_then(|extension| extension.to_str()),
        Some("json")
    );
}

#[test]
fn corrupt_entries_are_ignored() {
    let scratch = ScratchDirectory::new();
    let journal = Journal::at(&scratch.0);
    journal.begin(&entry(5)).unwrap();
    std::fs::write(scratch.0.join("operations/garbage.json"), b"{ not json").unwrap();
    assert_eq!(journal.pending().unwrap().len(), 1);
}

#[test]
fn table_snapshots_keep_the_last_ten_per_disk() {
    let scratch = ScratchDirectory::new();
    let journal = Journal::at(&scratch.0);
    let usb = test_inventory::usb_disk();
    let desktop = test_inventory::desktop_disk();
    (0..13).for_each(|taken_at| {
        journal
            .save_table(&TableSnapshot::of(&usb, 1_000 + taken_at))
            .unwrap();
    });
    let saved = journal.save_table(&TableSnapshot::of(&desktop, 5)).unwrap();
    assert_eq!(mode(&saved), 0o600);

    let kept = journal.tables(&usb.serial).unwrap();
    assert_eq!(kept.len(), 10);
    assert_eq!(
        kept.first().map(|snapshot| snapshot.taken_at_unix),
        Some(1_003)
    );
    assert_eq!(
        kept.last().map(|snapshot| snapshot.taken_at_unix),
        Some(1_012)
    );
    assert_eq!(kept[0].partitions.len(), 2);
    assert_eq!(journal.tables(&desktop.serial).unwrap().len(), 1);
}

#[test]
fn image_snapshots_are_keyed_by_a_safe_file_name() {
    let scratch = ScratchDirectory::new();
    let journal = Journal::at(&scratch.0);
    let mut image = test_inventory::usb_disk();
    image.backing_file = Some("/home/user/../stick.img".into());
    let saved = journal.save_table(&TableSnapshot::of(&image, 7)).unwrap();
    assert_eq!(saved.parent(), Some(scratch.0.join("tables").as_path()));
    assert_eq!(journal.tables("/home/user/../stick.img").unwrap().len(), 1);
}

#[test]
fn locate_ends_in_the_app_state_directory() {
    let located = Journal::locate().expect("home directory");
    assert!(located.root().ends_with("crownos/diskmanager"));
}
