use std::sync::Arc;
use std::time::Duration;

use zbus::{Fd, OwnedObjectPath};

use super::check::Inspection;
use super::udisks::{call, options};
use super::{OperationContext, check, format, image, mount, resize};
use crate::client::Session;
use crate::command::{FormatSpec, OperationId, PartitionScheme};
use crate::error::StorageError;
use crate::model::{BlockId, ByteSize, DiskId, FilesystemKind, Inventory};

const MANAGER_PATH: &str = "/org/freedesktop/UDisks2/Manager";
const MANAGER: &str = "org.freedesktop.UDisks2.Manager";
const IMAGE_SIZE: u64 = 4 << 30;
const SETTLE_INTERVAL: Duration = Duration::from_millis(100);
const SETTLE_ATTEMPTS: u32 = 100;

async fn settled<T>(session: &Session, find: impl Fn(&Inventory) -> Option<T>) -> T {
    for _ in 0..SETTLE_ATTEMPTS {
        if let Some(found) = find(&session.current()) {
            return found;
        }
        tokio::time::sleep(SETTLE_INTERVAL).await;
    }
    panic!("the inventory never showed the loop device's new state");
}

/// Exercises erase, check, shrink and grow on a sparse loop device this test creates itself.
/// Never touches another device, and never waits for a polkit prompt.
#[tokio::test]
#[ignore = "creates and modifies a loop device through UDisks2; run in a VM only"]
async fn erase_check_shrink_and_grow_a_loop_device() -> Result<(), StorageError> {
    let directory = std::env::temp_dir().join(format!("crownos-loop-{}", std::process::id()));
    std::fs::create_dir_all(&directory)?;
    let image_path = directory.join("disk.img");
    let image_file = std::fs::File::options()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&image_path)?;
    image_file.set_len(IMAGE_SIZE)?;

    let session = Session::connect().await?;
    let loop_block: OwnedObjectPath = call(
        &session,
        MANAGER_PATH,
        MANAGER,
        "LoopSetup",
        (Fd::from(&image_file), options()),
    )
    .await?;
    let disk_id = DiskId::new(loop_block.as_str());
    let context = OperationContext::new(OperationId(1), Arc::new(|_| {}));
    settled(&session, |inventory| inventory.disk(&disk_id).map(drop)).await;

    let spec = FormatSpec {
        filesystem: FilesystemKind::Ext4,
        label: "LoopTest".to_owned(),
        encryption: None,
    };
    format::erase_disk(
        &session,
        &context,
        None,
        &disk_id,
        PartitionScheme::Gpt,
        &spec,
    )
    .await?;
    let partition: BlockId = settled(&session, |inventory| {
        let disk = inventory.disk(&disk_id)?;
        disk.partitions()
            .find(|partition| partition.content.volume().is_some())
            .map(|partition| partition.block.clone())
    })
    .await;
    check::inspect(&session, &context, &partition, Inspection::Check).await?;

    mount::mount(&session, &partition).await?;
    session.refresh_usage();
    let bounds = settled(&session, |inventory| {
        crate::limits::resize_bounds(inventory, &partition)
            .ok()
            .filter(|bounds| bounds.min < bounds.current)
    })
    .await;
    let smaller = ByteSize(bounds.min.0.max(bounds.current.0 / 2));
    resize::resize(&session, &context, None, &partition, smaller, None).await?;
    let grown = settled(&session, |inventory| {
        crate::limits::resize_bounds(inventory, &partition)
            .ok()
            .filter(|bounds| bounds.current == smaller)
    })
    .await;
    resize::resize(&session, &context, None, &partition, grown.max, None).await?;

    image::close(&session, &disk_id).await?;
    std::fs::remove_dir_all(&directory)?;
    Ok(())
}
