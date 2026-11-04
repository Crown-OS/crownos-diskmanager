use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::{JournalEntry, TableSnapshot};
use crate::util::atomic_file::{create_private_directory, remove_if_present, write_private};

const STATE_SUBDIRECTORY: &str = "crownos/diskmanager";
const FALLBACK_STATE_HOME: &str = ".local/state";
const ENTRIES_DIRECTORY: &str = "operations";
const TABLES_DIRECTORY: &str = "tables";
const JSON_EXTENSION: &str = "json";
const KEPT_TABLE_SNAPSHOTS: usize = 10;

/// Interrupted-operation records and partition-table snapshots, stored as private JSON files.
#[derive(Debug, Clone)]
pub struct Journal {
    root: PathBuf,
}

impl Journal {
    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// `$XDG_STATE_HOME/crownos/diskmanager`, falling back to `~/.local/state`.
    pub fn locate() -> Option<Self> {
        let state_home = std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .or_else(|| std::env::home_dir().map(|home| home.join(FALLBACK_STATE_HOME)))?;
        Some(Self::at(state_home.join(STATE_SUBDIRECTORY)))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Records that `entry` is about to start.
    pub fn begin(&self, entry: &JournalEntry) -> io::Result<()> {
        self.write_entry(entry)
    }

    /// Marks one more step of `entry` as completed.
    pub fn advance(&self, entry: &mut JournalEntry) -> io::Result<()> {
        entry.completed_steps = entry.completed_steps.saturating_add(1);
        self.write_entry(entry)
    }

    /// Forgets `entry`, after it completed or was dismissed.
    pub fn finish(&self, entry: &JournalEntry) -> io::Result<()> {
        remove_if_present(&self.entry_path(entry))
    }

    /// Entries left behind by operations that never finished. Unreadable records are skipped.
    pub fn pending(&self) -> io::Result<Vec<JournalEntry>> {
        let mut entries: Vec<JournalEntry> = json_files(&self.root.join(ENTRIES_DIRECTORY))?
            .iter()
            .filter_map(|path| read_json(path))
            .collect();
        entries.sort_by_key(|entry| entry.started_at_unix);
        Ok(entries)
    }

    /// Saves `snapshot` and prunes older snapshots of the same disk beyond the last ten.
    pub fn save_table(&self, snapshot: &TableSnapshot) -> io::Result<PathBuf> {
        let directory = self.root.join(TABLES_DIRECTORY);
        create_private_directory(&directory)?;
        let disk_key = file_key(&snapshot.disk_serial);
        let path = directory.join(format!(
            "{disk_key}-{}.{JSON_EXTENSION}",
            snapshot.taken_at_unix
        ));
        write_private(&path, &serde_json::to_vec_pretty(snapshot)?)?;
        prune_tables(&directory, &disk_key)?;
        Ok(path)
    }

    /// Saved snapshots of the disk with `disk_serial`, oldest first.
    pub fn tables(&self, disk_serial: &str) -> io::Result<Vec<TableSnapshot>> {
        let directory = self.root.join(TABLES_DIRECTORY);
        Ok(snapshots_of(&directory, &file_key(disk_serial))?
            .iter()
            .filter_map(|(_, path)| read_json(path))
            .collect())
    }

    fn write_entry(&self, entry: &JournalEntry) -> io::Result<()> {
        create_private_directory(&self.root.join(ENTRIES_DIRECTORY))?;
        write_private(&self.entry_path(entry), &serde_json::to_vec_pretty(entry)?)
    }

    fn entry_path(&self, entry: &JournalEntry) -> PathBuf {
        let disk_key = file_key(entry.disk.object_path());
        self.root.join(ENTRIES_DIRECTORY).join(format!(
            "{disk_key}-{}.{JSON_EXTENSION}",
            entry.started_at_unix
        ))
    }
}

fn file_key(identity: &str) -> String {
    let key: String = identity
        .trim_matches('/')
        .chars()
        .map(
            |character| match character.is_ascii_alphanumeric() || character == '_' {
                true => character,
                false => '_',
            },
        )
        .collect();
    match key.is_empty() {
        true => "unknown".to_owned(),
        false => key,
    }
}

fn json_files(directory: &Path) -> io::Result<Vec<PathBuf>> {
    let listing = match fs::read_dir(directory) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        listing => listing?,
    };
    Ok(listing
        .filter_map(Result::ok)
        .map(|item| item.path())
        .filter(|path| path.extension() == Some(OsStr::new(JSON_EXTENSION)))
        .collect())
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
}

fn snapshots_of(directory: &Path, disk_key: &str) -> io::Result<Vec<(u64, PathBuf)>> {
    let mut snapshots: Vec<_> = json_files(directory)?
        .into_iter()
        .filter_map(|path| {
            let stem = path.file_stem()?.to_str()?;
            let (key, taken_at) = stem.rsplit_once('-')?;
            (key == disk_key).then_some((taken_at.parse().ok()?, path.clone()))
        })
        .collect();
    snapshots.sort_unstable_by_key(|(taken_at, _)| *taken_at);
    Ok(snapshots)
}

fn prune_tables(directory: &Path, disk_key: &str) -> io::Result<()> {
    let snapshots = snapshots_of(directory, disk_key)?;
    let excess = snapshots.len().saturating_sub(KEPT_TABLE_SNAPSHOTS);
    snapshots[..excess]
        .iter()
        .try_for_each(|(_, path)| remove_if_present(path))
}
