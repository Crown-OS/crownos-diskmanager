use super::completion::completed;
use super::format_options::{ensure_formattable, format_options};
use super::job;
use super::lookup::{block_target, disk};
use super::recorder::{Recorder, entry, save_table};
use super::udisks::{BLOCK, PARTITION_TABLE, call, call_when_ready, options};
use crate::client::Session;
use crate::command::{Completion, FormatSpec, PartitionScheme};
use crate::error::StorageError;
use crate::journal::{Journal, JournalOperation};
use crate::model::{BlockId, ByteSize, DiskId};
use crate::operations::OperationContext;

pub(crate) const FIRST_PARTITION_OFFSET: ByteSize = ByteSize::MIB;
const WHOLE_FREE_REGION: u64 = 0;
const DEFAULT_PARTITION_TYPE: &str = "";
const FORMATTING_PHASE: &str = "Formatting";
const ERASING_PHASE: &str = "Erasing";

/// Formats a volume. A new encryption layer replaces any existing one on the partition.
pub(crate) async fn format(
    session: &Session,
    context: &OperationContext,
    journal: Option<&Journal>,
    block: &BlockId,
    spec: &FormatSpec,
) -> Result<Completion, StorageError> {
    let inventory = session.current();
    ensure_formattable(&inventory, spec)?;
    let target = block_target(&inventory, block)?;
    let formatted = match target.is_cleartext() && spec.encryption.is_some() {
        true => &target.partition.block,
        false => target.block(),
    };
    let name = target.name();
    let recorder = Recorder::begin(
        journal,
        entry(
            JournalOperation::Format,
            target.disk,
            Some(formatted),
            name.clone(),
        ),
    )
    .await?;
    let path = formatted.object_path();
    let body = (spec.filesystem.udisks_type(), format_options(spec));
    job::follow(
        session,
        context,
        FORMATTING_PHASE,
        &[path],
        call::<_, ()>(session, path, BLOCK, "Format", body),
    )
    .await?;
    recorder.finish().await?;
    Ok(completed(format!(
        "{name} was formatted as {}.",
        spec.filesystem.short_name()
    )))
}

/// Replaces the partition table with an empty one holding a single partition that fills the disk.
pub(crate) async fn erase_disk(
    session: &Session,
    context: &OperationContext,
    journal: Option<&Journal>,
    id: &DiskId,
    scheme: PartitionScheme,
    spec: &FormatSpec,
) -> Result<Completion, StorageError> {
    let inventory = session.current();
    ensure_formattable(&inventory, spec)?;
    let disk = disk(&inventory, id)?;
    save_table(journal, disk).await?;
    let mut recorder = Recorder::begin(
        journal,
        entry(JournalOperation::EraseDisk, disk, None, disk.name.clone()),
    )
    .await?;
    let path = disk.block.object_path();
    let (table_type, partition_name) = match scheme {
        PartitionScheme::Gpt => ("gpt", spec.label.as_str()),
        PartitionScheme::Mbr => ("dos", ""),
    };
    let mut table_options = options();
    table_options.insert("tear-down", true.into());
    let new_table = call::<_, ()>(session, path, BLOCK, "Format", (table_type, table_options));
    job::follow(session, context, ERASING_PHASE, &[path], new_table).await?;
    recorder.advance().await?;
    let body = (
        FIRST_PARTITION_OFFSET.bytes(),
        WHOLE_FREE_REGION,
        DEFAULT_PARTITION_TYPE,
        partition_name,
        options(),
        spec.filesystem.udisks_type(),
        format_options(spec),
    );
    let partition = call_when_ready::<_, zbus::OwnedObjectPath>(
        session,
        path,
        PARTITION_TABLE,
        "CreatePartitionAndFormat",
        &body,
    );
    job::follow(session, context, FORMATTING_PHASE, &[path], partition).await?;
    recorder.finish().await?;
    Ok(completed(format!("{} was erased.", disk.name)))
}
