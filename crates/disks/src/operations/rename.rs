use super::completion::completed;
use super::lookup::block_target;
use super::udisks::{FILESYSTEM, SWAPSPACE, call, options};
use crate::client::Session;
use crate::command::Completion;
use crate::error::StorageError;
use crate::model::{BlockId, Content};

pub(crate) async fn rename(
    session: &Session,
    block: &BlockId,
    label: &str,
) -> Result<Completion, StorageError> {
    let inventory = session.current();
    let target = block_target(&inventory, block)?;
    let body = (label, options());
    match target.content() {
        Content::Swap { .. } => {
            call::<_, ()>(
                session,
                target.block().object_path(),
                SWAPSPACE,
                "SetLabel",
                body,
            )
            .await?
        }
        _ => {
            call::<_, ()>(
                session,
                target.filesystem_block().object_path(),
                FILESYSTEM,
                "SetLabel",
                body,
            )
            .await?
        }
    }
    Ok(completed(format!("Renamed to “{label}”.")))
}
