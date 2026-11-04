use std::fs::File;
use std::io;
use std::os::unix::fs::FileExt;

use super::copy::ChunkSink;
use crate::error::StorageError;
use crate::util::zero::is_all_zero;

const HOLE_GRANULE: usize = 64 << 10;

/// Writes a raw image, leaving holes where whole granules are zero.
pub(super) struct SparseFile<'a> {
    file: &'a File,
    position: u64,
}

impl<'a> SparseFile<'a> {
    pub(super) const fn new(file: &'a File) -> Self {
        Self { file, position: 0 }
    }

    pub(super) fn finish(self) -> io::Result<()> {
        self.file.set_len(self.position)
    }

    fn write_run(&self, run: &[u8], offset_in_chunk: usize) -> io::Result<()> {
        self.file
            .write_all_at(run, self.position + offset_in_chunk as u64)
    }
}

impl ChunkSink for SparseFile<'_> {
    fn write_chunk(&mut self, chunk: &[u8]) -> Result<(), StorageError> {
        let mut run_start = None;
        for (index, granule) in chunk.chunks(HOLE_GRANULE).enumerate() {
            let offset = index * HOLE_GRANULE;
            match (is_all_zero(granule), run_start) {
                (false, None) => run_start = Some(offset),
                (true, Some(start)) => {
                    self.write_run(&chunk[start..offset], start)?;
                    run_start = None;
                }
                _ => {}
            }
        }
        if let Some(start) = run_start {
            self.write_run(&chunk[start..], start)?;
        }
        self.position += chunk.len() as u64;
        Ok(())
    }
}
