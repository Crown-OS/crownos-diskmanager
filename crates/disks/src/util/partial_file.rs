use std::ffi::OsString;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

const PARTIAL_SUFFIX: &str = ".part";

/// A file written under `<destination>.part` that only takes its final name on [`commit`].
/// Dropping it uncommitted removes the partial file.
///
/// [`commit`]: PartialFile::commit
pub(crate) struct PartialFile {
    file: File,
    partial_path: PathBuf,
    destination: PathBuf,
    committed: bool,
}

impl PartialFile {
    pub(crate) fn create(destination: &Path) -> io::Result<Self> {
        let partial_path = partial_path(destination);
        let file = File::options()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&partial_path)?;
        Ok(Self {
            file,
            partial_path,
            destination: destination.to_path_buf(),
            committed: false,
        })
    }

    pub(crate) fn file(&self) -> &File {
        &self.file
    }

    pub(crate) fn commit(mut self) -> io::Result<()> {
        self.file.sync_all()?;
        std::fs::rename(&self.partial_path, &self.destination)?;
        self.committed = true;
        match self
            .destination
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            Some(parent) => File::open(parent)?.sync_all(),
            None => Ok(()),
        }
    }
}

impl Drop for PartialFile {
    fn drop(&mut self) {
        if !self.committed {
            drop(std::fs::remove_file(&self.partial_path));
        }
    }
}

pub(crate) fn partial_path(destination: &Path) -> PathBuf {
    let mut path = OsString::from(destination.as_os_str());
    path.push(PARTIAL_SUFFIX);
    PathBuf::from(path)
}
