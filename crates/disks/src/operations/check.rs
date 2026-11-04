use super::completion::completed;
use super::job;
use super::lookup::block_target;
use super::udisks::{FILESYSTEM, call, options};
use crate::client::Session;
use crate::command::Completion;
use crate::error::StorageError;
use crate::model::BlockId;
use crate::operations::OperationContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Inspection {
    Check,
    Repair,
}

impl Inspection {
    const fn method(self) -> &'static str {
        match self {
            Self::Check => "Check",
            Self::Repair => "Repair",
        }
    }

    const fn phase(self) -> &'static str {
        match self {
            Self::Check => "Checking",
            Self::Repair => "Repairing",
        }
    }
}

/// Runs `Filesystem.Check` on `path`, reporting whether the filesystem is consistent.
pub(crate) async fn is_consistent(
    session: &Session,
    context: &OperationContext,
    path: &str,
    inspection: Inspection,
) -> Result<bool, StorageError> {
    let work = call(session, path, FILESYSTEM, inspection.method(), (options(),));
    job::follow(session, context, inspection.phase(), &[path], work).await
}

pub(crate) async fn inspect(
    session: &Session,
    context: &OperationContext,
    block: &BlockId,
    inspection: Inspection,
) -> Result<Completion, StorageError> {
    let inventory = session.current();
    let target = block_target(&inventory, block)?;
    let name = target.name();
    let healthy = is_consistent(
        session,
        context,
        target.filesystem_block().object_path(),
        inspection,
    )
    .await?;
    Ok(completed(match (inspection, healthy) {
        (Inspection::Check, true) => format!("No problems found on {name}."),
        (Inspection::Check, false) => format!("{name} has errors. Repair it to fix them."),
        (Inspection::Repair, true) => format!("{name} was repaired."),
        (Inspection::Repair, false) => format!("Some problems on {name} couldn't be repaired."),
    }))
}
