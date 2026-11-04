use zbus::{OwnedObjectPath, Variant};

use super::completion::completed;
use super::format_options::{ensure_formattable, format_options};
use super::job;
use super::lookup::{block_target, disk};
use super::recorder::save_table;
use super::udisks::{PARTITION, PARTITION_TABLE, call, options};
use crate::client::Session;
use crate::command::{Completion, FormatSpec};
use crate::error::StorageError;
use crate::guard::Refusal;
use crate::journal::Journal;
use crate::model::{BlockId, ByteSize, Disk, DiskId, Region, TableKind};
use crate::operations::OperationContext;

const FILL_REGION: u64 = 0;
const DEFAULT_PARTITION_TYPE: &str = "";
const CREATING_PHASE: &str = "Creating partition";

/// Where a requested span sits inside the free region containing it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fit {
    ReachesEnd,
    Inside,
}

fn fit(disk: &Disk, offset: ByteSize, size: ByteSize) -> Option<Fit> {
    let end = offset.bytes().checked_add(size.bytes())?;
    disk.layout.iter().find_map(|region| match region {
        Region::Free {
            offset: free,
            size: free_size,
        } if free.bytes() <= offset.bytes() => {
            let free_end = free.bytes() + free_size.bytes();
            match end.cmp(&free_end) {
                std::cmp::Ordering::Greater => None,
                std::cmp::Ordering::Equal => Some(Fit::ReachesEnd),
                std::cmp::Ordering::Less => Some(Fit::Inside),
            }
        }
        _ => None,
    })
}

fn supports_names(disk: &Disk) -> bool {
    disk.table == Some(TableKind::Gpt)
}

/// Creates and formats a partition in free space.
pub(crate) async fn create(
    session: &Session,
    context: &OperationContext,
    journal: Option<&Journal>,
    id: &DiskId,
    (offset, size): (ByteSize, ByteSize),
    name: &str,
    spec: &FormatSpec,
) -> Result<Completion, StorageError> {
    let inventory = session.current();
    ensure_formattable(&inventory, spec)?;
    let disk = disk(&inventory, id)?;
    let requested_size = match fit(disk, offset, size).ok_or(Refusal::NoRoom)? {
        Fit::ReachesEnd => FILL_REGION,
        Fit::Inside => size.bytes(),
    };
    save_table(journal, disk).await?;
    let partition_name = if supports_names(disk) { name } else { "" };
    let path = disk.block.object_path();
    let body = (
        offset.bytes(),
        requested_size,
        DEFAULT_PARTITION_TYPE,
        partition_name,
        options(),
        spec.filesystem.udisks_type(),
        format_options(spec),
    );
    let created = call::<_, OwnedObjectPath>(
        session,
        path,
        PARTITION_TABLE,
        "CreatePartitionAndFormat",
        body,
    );
    job::follow(session, context, CREATING_PHASE, &[path], created).await?;
    Ok(completed(format!(
        "Created a {} partition on {}.",
        spec.filesystem.short_name(),
        disk.name
    )))
}

/// Deletes a partition after saving the table layout to the journal.
pub(crate) async fn delete(
    session: &Session,
    journal: Option<&Journal>,
    block: &BlockId,
) -> Result<Completion, StorageError> {
    let inventory = session.current();
    let target = block_target(&inventory, block)?;
    save_table(journal, target.disk).await?;
    let mut delete_options = options();
    delete_options.insert("tear-down", Variant::from(true));
    call::<_, ()>(
        session,
        target.partition.block.object_path(),
        PARTITION,
        "Delete",
        (delete_options,),
    )
    .await?;
    Ok(completed(format!("{} was deleted.", target.name())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guard::test_inventory::{desktop_disk, gib};

    #[test]
    fn spans_must_lie_inside_one_free_region() {
        let disk = desktop_disk();
        let Some(Region::Free { offset, size }) = disk.layout.get(3).cloned() else {
            panic!("free region expected");
        };
        assert_eq!(fit(&disk, offset, size), Some(Fit::ReachesEnd));
        assert_eq!(fit(&disk, offset, gib(1)), Some(Fit::Inside));
        assert_eq!(fit(&disk, offset, ByteSize(size.0 + 1)), None);
        assert_eq!(fit(&disk, ByteSize(offset.0 - 1), gib(1)), None);
    }
}
