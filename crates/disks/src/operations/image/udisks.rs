use std::collections::HashMap;
use std::fs::File;
use std::os::fd::OwnedFd;
use std::time::Duration;

use rustix::fs::OFlags;
use udisks2::block::BlockProxy;
use udisks2::encrypted::EncryptedProxy;
use udisks2::filesystem::FilesystemProxy;
use udisks2::r#loop::LoopProxy;
use udisks2::swapspace::SwapspaceProxy;
use zbus::proxy::{CacheProperties, Defaults, ProxyImpl};
use zbus::{Fd, ObjectPath, OwnedObjectPath, Proxy, Variant};

use super::objects::{ENCRYPTED, FILESYSTEM, ObjectTree};
use crate::client::Session;
use crate::error::StorageError;
use crate::util::direct_io;

const EXCLUSIVE_OPEN_FLAGS: i32 = OFlags::EXCL.union(OFlags::CLOEXEC).bits().cast_signed();
const PARTITION_PROBE_INTERVAL: Duration = Duration::from_millis(200);
const PARTITION_PROBE_ATTEMPTS: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AccessMode {
    Read,
    ReadWrite,
}

impl AccessMode {
    const fn as_udisks(self) -> &'static str {
        match self {
            Self::Read => "r",
            Self::ReadWrite => "rw",
        }
    }
}

fn interactive_options() -> HashMap<&'static str, Variant<'static>> {
    udisks2::standard_options(false)
}

async fn proxy<P>(session: &Session, path: &ObjectPath<'_>) -> Result<P, StorageError>
where
    P: ProxyImpl<'static> + From<Proxy<'static>> + Defaults,
{
    let connection = session.client.object_manager().inner().connection();
    Ok(P::builder(connection)
        .path(path.as_str().to_owned())
        .cache_properties(CacheProperties::No)
        .build()
        .await?)
}

/// Opens the device exclusively through UDisks2 (polkit `open-device`) and enables `O_DIRECT` when possible.
pub(super) async fn open_device(
    session: &Session,
    block: &ObjectPath<'_>,
    mode: AccessMode,
) -> Result<File, StorageError> {
    let block: BlockProxy<'static> = proxy(session, block).await?;
    let mut options = interactive_options();
    options.insert("flags", Variant::from(EXCLUSIVE_OPEN_FLAGS));
    let file = File::from(OwnedFd::from(
        block.open_device(mode.as_udisks(), options).await?,
    ));
    direct_io::enable(&file);
    Ok(file)
}

pub(super) async fn rescan(session: &Session, block: &ObjectPath<'_>) -> Result<(), StorageError> {
    let block: BlockProxy<'static> = proxy(session, block).await?;
    Ok(block.rescan(interactive_options()).await?)
}

pub(super) async fn loop_setup(
    session: &Session,
    image: &File,
) -> Result<OwnedObjectPath, StorageError> {
    let mut options = interactive_options();
    options.insert("read-only", Variant::from(true));
    Ok(session
        .client
        .manager()
        .loop_setup(Fd::from(image), options)
        .await?)
}

pub(super) async fn delete_loop(
    session: &Session,
    block: &ObjectPath<'_>,
) -> Result<(), StorageError> {
    let loop_device: LoopProxy<'static> = proxy(session, block).await?;
    Ok(loop_device.delete(interactive_options()).await?)
}

/// Unmounts, deactivates swap and locks everything stacked on `root`, innermost first.
pub(super) async fn tear_down(
    session: &Session,
    root: &ObjectPath<'_>,
) -> Result<(), StorageError> {
    let tree = ObjectTree::fetch(session).await?;
    for block in tree.stack(root).iter().rev() {
        if tree.is_mounted(block) {
            let filesystem: FilesystemProxy<'static> = proxy(session, block).await?;
            filesystem.unmount(interactive_options()).await?;
        }
        if tree.is_active_swap(block) {
            let swap: SwapspaceProxy<'static> = proxy(session, block).await?;
            swap.stop(interactive_options()).await?;
        }
        if tree.has_interface(block, ENCRYPTED) && tree.cleartext(block).is_some() {
            let encrypted: EncryptedProxy<'static> = proxy(session, block).await?;
            encrypted.lock(interactive_options()).await?;
        }
    }
    Ok(())
}

/// Mounts the filesystems of a freshly attached loop device: its partitions' if it has any, else its own.
pub(super) async fn mount_filesystems(
    session: &Session,
    root: &ObjectPath<'_>,
) -> Result<Vec<String>, StorageError> {
    let tree = settled_tree(session, root).await?;
    let partitions: Vec<_> = tree.partitions(root).collect();
    let candidates = match partitions.is_empty() {
        true => vec![root.clone().into()],
        false => partitions,
    };
    let mut mount_points = Vec::new();
    for block in candidates
        .iter()
        .filter(|block| tree.has_interface(block, FILESYSTEM) && !tree.is_mounted(block))
    {
        let filesystem: FilesystemProxy<'static> = proxy(session, block).await?;
        mount_points.push(filesystem.mount(interactive_options()).await?);
    }
    Ok(mount_points)
}

async fn settled_tree(
    session: &Session,
    root: &ObjectPath<'_>,
) -> Result<ObjectTree, StorageError> {
    let mut tree = ObjectTree::fetch(session).await?;
    if !tree.has_partition_table(root) {
        return Ok(tree);
    }
    for _ in 0..PARTITION_PROBE_ATTEMPTS {
        tokio::time::sleep(PARTITION_PROBE_INTERVAL).await;
        let next = ObjectTree::fetch(session).await?;
        let settled = next.partitions(root).count() == tree.partitions(root).count();
        tree = next;
        if settled {
            break;
        }
    }
    Ok(tree)
}
