use crate::error::StorageError;
use crate::guard::target::{BlockTarget, Resolved, resolve_block};
use crate::model::{BlockId, Disk, DiskId, Inventory};

const GONE_VOLUME: &str = "The volume";
const GONE_DISK: &str = "The disk";

pub(crate) fn block_target<'a>(
    inventory: &'a Inventory,
    block: &BlockId,
) -> Result<BlockTarget<'a>, StorageError> {
    match resolve_block(inventory, block) {
        Some(Resolved::Block(target)) => Ok(target),
        _ => Err(StorageError::Gone(GONE_VOLUME.to_owned())),
    }
}

pub(crate) fn disk<'a>(inventory: &'a Inventory, id: &DiskId) -> Result<&'a Disk, StorageError> {
    inventory
        .disk(id)
        .ok_or_else(|| StorageError::Gone(GONE_DISK.to_owned()))
}
