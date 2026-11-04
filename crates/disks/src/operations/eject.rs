use super::completion::completed;
use super::lookup::disk;
use super::teardown::release_partition;
use super::udisks::{DRIVE, call, options};
use crate::client::Session;
use crate::command::Completion;
use crate::error::StorageError;
use crate::model::{DiskId, DiskKind, Removal};
use crate::operations::image;

/// Unmounts every filesystem on the disk, locks its encrypted volumes, then powers it off,
/// ejects it, or detaches it when it is a disk image. A failing step leaves the rest mounted.
pub(crate) async fn eject(session: &Session, id: &DiskId) -> Result<Completion, StorageError> {
    let inventory = session.current();
    let disk = disk(&inventory, id)?;
    for partition in disk.partitions() {
        release_partition(session, partition).await?;
    }
    if disk.kind == DiskKind::Image {
        return image::close(session, id).await;
    }
    let method = match disk.removal {
        Removal::Removable {
            can_power_off: true,
            ..
        } => "PowerOff",
        Removal::Removable { .. } | Removal::Fixed => "Eject",
    };
    call::<_, ()>(session, id.object_path(), DRIVE, method, (options(),)).await?;
    Ok(completed(format!("Safe to remove {}.", disk.name)))
}
