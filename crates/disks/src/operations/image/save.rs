use std::path::Path;

use super::codec;
use super::copy::{ChunkSource, Transfer};
use super::sparse::SparseFile;
use crate::command::Compression;
use crate::error::StorageError;
use crate::operations::OperationContext;
use crate::util::partial_file::PartialFile;

const SAVING_PHASE: &str = "Saving image";

/// Streams `source` into `destination`, which only appears once the image is complete.
pub(super) fn save_image(
    source: impl ChunkSource + Send,
    source_size: u64,
    destination: &Path,
    compression: Compression,
    context: &OperationContext,
    buffer_size: usize,
) -> Result<(), StorageError> {
    let partial = PartialFile::create(destination)?;
    let file = partial.file();
    let transfer = Transfer {
        context,
        phase: SAVING_PHASE,
        expected_bytes: Some(source_size),
        buffer_size,
    };
    match compression {
        Compression::None => {
            let mut sink = SparseFile::new(file);
            transfer.run(source, &mut sink)?;
            sink.finish()?;
        }
        Compression::Zstd => {
            let mut encoder = codec::zstd_encoder(file, source_size)?;
            transfer.run(source, &mut encoder)?;
            encoder.finish()?;
        }
        Compression::Xz => {
            let mut encoder = codec::xz_encoder(file)?;
            transfer.run(source, &mut encoder)?;
            encoder.finish()?;
        }
    }
    Ok(partial.commit()?)
}
