use super::udisks::{ENCRYPTED, FILESYSTEM, SWAPSPACE, call, options};
use crate::client::Session;
use crate::error::StorageError;
use crate::guard::target::unlocked;
use crate::model::{Content, Partition, Unlocked};

/// Unmounts the filesystem on `path`. Never forces, so busy volumes report DeviceBusy.
pub(crate) async fn unmount(session: &Session, path: &str) -> Result<(), StorageError> {
    call(session, path, FILESYSTEM, "Unmount", (options(),)).await
}

pub(crate) async fn mount(session: &Session, path: &str) -> Result<String, StorageError> {
    call(session, path, FILESYSTEM, "Mount", (options(),)).await
}

pub(crate) async fn lock(session: &Session, path: &str) -> Result<(), StorageError> {
    call(session, path, ENCRYPTED, "Lock", (options(),)).await
}

/// Unmounts or deactivates whatever uses `path` directly.
async fn release_content(
    session: &Session,
    path: &str,
    content: &Content,
) -> Result<(), StorageError> {
    match content {
        Content::Filesystem(volume) if volume.is_mounted() => unmount(session, path).await,
        Content::Swap { active: true } => {
            call(session, path, SWAPSPACE, "Stop", (options(),)).await
        }
        _ => Ok(()),
    }
}

/// Releases the cleartext device of an unlocked volume so it can be locked.
pub(crate) async fn release_cleartext(
    session: &Session,
    unlocked: &Unlocked,
) -> Result<(), StorageError> {
    release_content(session, unlocked.cleartext.object_path(), &unlocked.content).await
}

/// Unmounts, deactivates and locks everything on `partition`, innermost first.
pub(crate) async fn release_partition(
    session: &Session,
    partition: &Partition,
) -> Result<(), StorageError> {
    let path = partition.block.object_path();
    match unlocked(&partition.content) {
        Some(unlocked) => {
            release_cleartext(session, unlocked).await?;
            lock(session, path).await
        }
        None => release_content(session, path, &partition.content).await,
    }
}
