use zbus::fdo::ManagedObjects;
use zbus::{ObjectPath, OwnedObjectPath, Variant};

use crate::client::Session;
use crate::error::StorageError;

const BLOCK: &str = "org.freedesktop.UDisks2.Block";
const PARTITION: &str = "org.freedesktop.UDisks2.Partition";
const PARTITION_TABLE: &str = "org.freedesktop.UDisks2.PartitionTable";
pub(super) const FILESYSTEM: &str = "org.freedesktop.UDisks2.Filesystem";
pub(super) const ENCRYPTED: &str = "org.freedesktop.UDisks2.Encrypted";
const SWAPSPACE: &str = "org.freedesktop.UDisks2.Swapspace";

/// One snapshot of the UDisks2 object tree, used to find everything stacked on a block device.
pub(super) struct ObjectTree(ManagedObjects);

impl ObjectTree {
    pub(super) async fn fetch(session: &Session) -> Result<Self, StorageError> {
        let objects = session.client.object_manager().get_managed_objects().await;
        Ok(Self(objects.map_err(zbus::Error::from)?))
    }

    /// The block, its partitions and every unlocked cleartext device on them, outermost first.
    pub(super) fn stack(&self, root: &ObjectPath<'_>) -> Vec<OwnedObjectPath> {
        let mut stack: Vec<OwnedObjectPath> = vec![root.clone().into()];
        stack.extend(self.partitions(root));
        let mut index = 0;
        while let Some(block) = stack.get(index) {
            if let Some(cleartext) = self.cleartext(block) {
                stack.push(cleartext);
            }
            index += 1;
        }
        stack
    }

    pub(super) fn has_partition_table(&self, block: &ObjectPath<'_>) -> bool {
        self.has_interface(block, PARTITION_TABLE)
    }

    pub(super) fn partitions<'a>(
        &'a self,
        table: &'a ObjectPath<'_>,
    ) -> impl Iterator<Item = OwnedObjectPath> + 'a {
        self.0
            .keys()
            .filter(move |path| {
                self.object_path(path, PARTITION, "Table")
                    .is_some_and(|parent| &parent == table)
            })
            .cloned()
    }

    pub(super) fn has_interface(&self, path: &ObjectPath<'_>, interface: &str) -> bool {
        self.0
            .get(path)
            .is_some_and(|interfaces| interfaces.keys().any(|name| name.as_str() == interface))
    }

    pub(super) fn is_mounted(&self, path: &ObjectPath<'_>) -> bool {
        self.property(path, FILESYSTEM, "MountPoints").is_some_and(
            |mount_points| matches!(mount_points, Variant::Array(array) if !array.is_empty()),
        )
    }

    pub(super) fn is_active_swap(&self, path: &ObjectPath<'_>) -> bool {
        self.property(path, SWAPSPACE, "Active")
            .is_some_and(|active| active.downcast_ref::<bool>().unwrap_or(false))
    }

    pub(super) fn cleartext(&self, backing: &ObjectPath<'_>) -> Option<OwnedObjectPath> {
        self.0
            .keys()
            .find(|path| {
                self.object_path(path, BLOCK, "CryptoBackingDevice")
                    .is_some_and(|parent| &parent == backing)
            })
            .cloned()
    }

    fn object_path(
        &self,
        path: &ObjectPath<'_>,
        interface: &str,
        name: &str,
    ) -> Option<ObjectPath<'_>> {
        self.property(path, interface, name)?
            .downcast_ref::<ObjectPath<'_>>()
            .ok()
    }

    fn property(&self, path: &ObjectPath<'_>, interface: &str, name: &str) -> Option<&Variant<'_>> {
        self.0
            .get(path)?
            .iter()
            .find(|(interface_name, _)| interface_name.as_str() == interface)?
            .1
            .get(name)
            .map(|value| &**value)
    }
}
