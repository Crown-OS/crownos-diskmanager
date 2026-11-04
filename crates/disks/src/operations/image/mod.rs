mod codec;
mod copy;
mod device;
mod format;
mod gpt;
mod objects;
mod progress;
mod save;
mod sparse;
mod target;
mod udisks;
mod write;
mod xz_index;

#[cfg(test)]
mod tests;

use std::borrow::Cow;
use std::ffi::OsStr;
use std::path::Path;

use zbus::ObjectPath;

use self::copy::BUFFER_SIZE;
use self::device::DeviceReader;
use self::format::ImageFile;
use self::udisks::AccessMode;
use self::write::RestorePlan;
use crate::client::Session;
use crate::command::{Completion, Compression, Progress, Target};
use crate::error::StorageError;
use crate::model::{BlockId, DiskId};
use crate::operations::OperationContext;
use crate::util::block_size::device_size;
use crate::util::blocking::run_blocking;

const PREPARING_PHASE: &str = "Preparing";

pub(crate) async fn create(
    session: &Session,
    context: &OperationContext,
    source: &Target,
    destination: &Path,
    compression: Compression,
) -> Result<Completion, StorageError> {
    let source = target::resolve(&session.current(), source)?;
    let device =
        udisks::open_device(session, &object_path(&source.block)?, AccessMode::Read).await?;
    let destination = destination.to_path_buf();
    let context = context.clone();
    run_blocking(move || {
        let size = device_size(&device)?;
        save::save_image(
            DeviceReader::new(&device, size),
            size,
            &destination,
            compression,
            &context,
            BUFFER_SIZE,
        )
    })
    .await?;
    Ok(completion(format!("Saved image of {}", source.name)))
}

pub(crate) async fn restore(
    session: &Session,
    context: &OperationContext,
    image: &Path,
    target: &Target,
    verify: bool,
) -> Result<Completion, StorageError> {
    let target = target::resolve(&session.current(), target)?;
    let image_path = image.to_path_buf();
    let image_file = run_blocking(move || Ok(ImageFile::open(&image_path)?)).await?;
    image_file.ensure_fits(target.capacity.bytes())?;
    context.ensure_not_cancelled()?;
    context.report(PREPARING_PHASE, Progress::default());
    let block = object_path(&target.block)?;
    udisks::tear_down(session, &block).await?;
    let device = udisks::open_device(session, &block, AccessMode::ReadWrite).await?;
    let plan = RestorePlan {
        verification: verify.into(),
        layout: target.layout,
        buffer_size: BUFFER_SIZE,
    };
    let worker_context = context.clone();
    run_blocking(move || write::write_image(image_file, &device, plan, &worker_context)).await?;
    udisks::rescan(session, &block).await.ok();
    Ok(completion(format!(
        "Restored {} to {}",
        file_name(image),
        target.name
    )))
}

pub(crate) async fn open(session: &Session, image: &Path) -> Result<Completion, StorageError> {
    let file = tokio::fs::File::open(image).await?.into_std().await;
    let loop_block = udisks::loop_setup(session, &file).await?;
    drop(file);
    udisks::mount_filesystems(session, &loop_block).await?;
    Ok(completion(format!("Opened {}", file_name(image))))
}

pub(crate) async fn close(session: &Session, disk: &DiskId) -> Result<Completion, StorageError> {
    let inventory = session.current();
    let disk = inventory
        .disk(disk)
        .ok_or_else(|| StorageError::Gone(String::from("The disk image")))?;
    let block = object_path(&disk.block)?;
    udisks::tear_down(session, &block).await?;
    udisks::delete_loop(session, &block).await?;
    let name = disk
        .backing_file
        .as_deref()
        .map_or(Cow::Borrowed(disk.name.as_str()), file_name);
    Ok(completion(format!("Closed {name}")))
}

fn object_path(block: &BlockId) -> Result<ObjectPath<'_>, StorageError> {
    Ok(ObjectPath::try_from(block.object_path())?)
}

fn file_name(path: &Path) -> Cow<'_, str> {
    path.file_name()
        .map_or_else(|| path.to_string_lossy(), OsStr::to_string_lossy)
}

fn completion(message: String) -> Completion {
    Completion { message }
}
