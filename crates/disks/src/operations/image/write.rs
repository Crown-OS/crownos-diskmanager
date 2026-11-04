use std::fs::File;

use super::copy::Transfer;
use super::device::{self, DeviceReader, DeviceWriter, Verification};
use super::format::ImageFile;
use super::gpt;
use super::target::TargetLayout;
use crate::error::StorageError;
use crate::operations::OperationContext;
use crate::util::block_size::device_size;

const WRITING_PHASE: &str = "Writing";
const VERIFYING_PHASE: &str = "Verifying";

#[derive(Debug, Clone, Copy)]
pub(super) struct RestorePlan {
    pub verification: Verification,
    pub layout: TargetLayout,
    pub buffer_size: usize,
}

/// Writes the decompressed image to the device, then verifies it and fixes up a GPT as planned.
pub(super) fn write_image(
    image: ImageFile,
    device: &File,
    plan: RestorePlan,
    context: &OperationContext,
) -> Result<u64, StorageError> {
    let capacity = device_size(device)?;
    image.ensure_fits(capacity)?;
    let transfer = Transfer {
        context,
        phase: WRITING_PHASE,
        expected_bytes: image.decompressed_size,
        buffer_size: plan.buffer_size,
    };
    let mut writer = DeviceWriter::new(device, capacity, plan.verification);
    let written = transfer.run(image.into_reader()?, &mut writer)?;
    device::flush(device, written)?;
    if let Some(digest) = writer.digest() {
        verify(device, written, digest, context, plan.buffer_size)?;
    }
    if plan.layout == TargetLayout::WholeDisk && written < capacity {
        gpt::relocate_backup_header(device)?;
    }
    Ok(written)
}

pub(super) fn verify(
    device: &File,
    written: u64,
    expected: blake3::Hash,
    context: &OperationContext,
    buffer_size: usize,
) -> Result<(), StorageError> {
    let transfer = Transfer {
        context,
        phase: VERIFYING_PHASE,
        expected_bytes: Some(written),
        buffer_size,
    };
    let mut hasher = blake3::Hasher::new();
    transfer.run(DeviceReader::new(device, written), &mut hasher)?;
    match hasher.finalize() == expected {
        true => Ok(()),
        false => Err(StorageError::VerificationFailed),
    }
}
