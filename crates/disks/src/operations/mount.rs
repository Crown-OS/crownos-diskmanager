use super::completion::completed;
use super::lookup::block_target;
use super::teardown;
use crate::client::Session;
use crate::command::Completion;
use crate::error::StorageError;
use crate::model::BlockId;

pub(crate) async fn mount(session: &Session, block: &BlockId) -> Result<Completion, StorageError> {
    let inventory = session.current();
    let target = block_target(&inventory, block)?;
    let mount_point = teardown::mount(session, target.filesystem_block().object_path()).await?;
    Ok(completed(format!(
        "{} is mounted at {mount_point}.",
        target.name()
    )))
}

pub(crate) async fn unmount(
    session: &Session,
    block: &BlockId,
) -> Result<Completion, StorageError> {
    let inventory = session.current();
    let target = block_target(&inventory, block)?;
    teardown::unmount(session, target.filesystem_block().object_path()).await?;
    Ok(completed(format!("{} is unmounted.", target.name())))
}
