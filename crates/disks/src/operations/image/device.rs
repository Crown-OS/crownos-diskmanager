use std::fs::File;
use std::io;
use std::num::NonZeroU64;
use std::os::unix::fs::FileExt;

use rustix::fs::{Advice, fadvise};

use super::copy::ChunkSink;
use crate::error::StorageError;
use crate::model::ByteSize;
use crate::util::aligned_buffer::PAGE_SIZE;
use crate::util::direct_io;

/// Reads the first `end` bytes of a device in page-aligned requests.
pub(super) struct DeviceReader<'a> {
    device: &'a File,
    position: u64,
    end: u64,
}

impl<'a> DeviceReader<'a> {
    pub(super) const fn new(device: &'a File, end: u64) -> Self {
        Self {
            device,
            position: 0,
            end,
        }
    }
}

impl super::copy::ChunkSource for DeviceReader<'_> {
    fn fill(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let wanted = usize::try_from(self.end - self.position)
            .map_or(buffer.len(), |left| left.min(buffer.len()));
        let request = wanted.next_multiple_of(PAGE_SIZE).min(buffer.len());
        let mut filled = 0;
        while filled < wanted {
            let read = direct_io::with_fallback(self.device, || {
                self.device
                    .read_at(&mut buffer[filled..request], self.position + filled as u64)
            })?;
            if read == 0 {
                return Err(io::ErrorKind::UnexpectedEof.into());
            }
            filled += read;
        }
        self.position += wanted as u64;
        Ok(wanted)
    }
}

/// Writes an image to a device, refusing to run past its end and optionally hashing what it wrote.
pub(super) struct DeviceWriter<'a> {
    device: &'a File,
    position: u64,
    capacity: u64,
    hasher: Option<blake3::Hasher>,
}

impl<'a> DeviceWriter<'a> {
    pub(super) fn new(device: &'a File, capacity: u64, verification: Verification) -> Self {
        let hasher = match verification {
            Verification::ReadBack => Some(blake3::Hasher::new()),
            Verification::Skip => None,
        };
        Self {
            device,
            position: 0,
            capacity,
            hasher,
        }
    }

    pub(super) fn digest(&self) -> Option<blake3::Hash> {
        self.hasher.as_ref().map(blake3::Hasher::finalize)
    }
}

impl ChunkSink for DeviceWriter<'_> {
    fn write_chunk(&mut self, chunk: &[u8]) -> Result<(), StorageError> {
        let end = self.position + chunk.len() as u64;
        if end > self.capacity {
            return Err(StorageError::ImageTooLarge {
                capacity: ByteSize(self.capacity),
            });
        }
        direct_io::with_fallback(self.device, || {
            self.device.write_all_at(chunk, self.position)
        })?;
        if let Some(hasher) = &mut self.hasher {
            hasher.update(chunk);
        }
        self.position = end;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Verification {
    Skip,
    ReadBack,
}

impl From<bool> for Verification {
    fn from(verify: bool) -> Self {
        match verify {
            true => Self::ReadBack,
            false => Self::Skip,
        }
    }
}

/// Makes the written range durable and evicts it from the page cache, so a read-back hits the disk.
pub(super) fn flush(device: &File, written: u64) -> io::Result<()> {
    device.sync_all()?;
    match NonZeroU64::new(written) {
        Some(len) => fadvise(device, 0, Some(len), Advice::DontNeed).map_err(io::Error::from),
        None => Ok(()),
    }
}
