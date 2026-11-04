use zbus::OwnedObjectPath;

use super::completion::completed;
use super::lookup::block_target;
use super::teardown::{lock as lock_block, release_cleartext};
use super::udisks::{ENCRYPTED, FILESYSTEM, call, call_when_ready, options};
use crate::client::Session;
use crate::command::{Completion, Secret};
use crate::error::StorageError;
use crate::guard::target::unlocked;
use crate::model::BlockId;

/// Unlocks the volume and mounts the filesystem inside it, when there is one.
pub(crate) async fn unlock(
    session: &Session,
    block: &BlockId,
    passphrase: &Secret,
) -> Result<Completion, StorageError> {
    let inventory = session.current();
    let target = block_target(&inventory, block)?;
    let name = target.name();
    let cleartext: OwnedObjectPath = call(
        session,
        target.partition.block.object_path(),
        ENCRYPTED,
        "Unlock",
        (passphrase.expose(), options()),
    )
    .await?;
    let mounted = call_when_ready::<_, String>(
        session,
        cleartext.as_str(),
        FILESYSTEM,
        "Mount",
        &(options(),),
    )
    .await;
    Ok(completed(match mounted {
        Ok(mount_point) => format!("{name} is unlocked and mounted at {mount_point}."),
        Err(_) => format!("{name} is unlocked."),
    }))
}

/// Unmounts the filesystem inside the volume, then locks it.
pub(crate) async fn lock(session: &Session, block: &BlockId) -> Result<Completion, StorageError> {
    let inventory = session.current();
    let target = block_target(&inventory, block)?;
    if let Some(unlocked) = unlocked(&target.partition.content) {
        release_cleartext(session, unlocked).await?;
    }
    lock_block(session, target.partition.block.object_path()).await?;
    Ok(completed(format!("{} is locked.", target.name())))
}
