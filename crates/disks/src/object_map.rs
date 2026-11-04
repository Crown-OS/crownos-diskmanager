//! A pure, serialisable mirror of UDisks2's `GetManagedObjects` plus the few
//! method-call results the snapshot needs.

mod capabilities;
mod value;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub use capabilities::{FormatProbe, ManagerCapabilities, ResizeProbe};
pub use value::Value;

/// Interface names are stored without this prefix to keep fixtures readable.
pub const UDISKS_INTERFACE_PREFIX: &str = "org.freedesktop.UDisks2.";

/// The interfaces the snapshot reads; everything else is dropped on import.
pub const TRACKED_INTERFACES: [&str; 10] = [
    interface::BLOCK,
    interface::DRIVE,
    interface::ATA,
    interface::NVME_CONTROLLER,
    interface::PARTITION,
    interface::PARTITION_TABLE,
    interface::FILESYSTEM,
    interface::ENCRYPTED,
    interface::SWAPSPACE,
    interface::LOOP,
];

/// Bulky properties nothing reads; dropped on import to save memory.
pub const IGNORED_PROPERTIES: [&str; 8] = [
    "Configuration",
    "Symlinks",
    "UserspaceMountOptions",
    "MediaCompatibility",
    "SubsystemNQN",
    "TimeDetected",
    "TimeMediaDetected",
    "temp_sensors",
];

pub mod interface {
    pub const BLOCK: &str = "Block";
    pub const DRIVE: &str = "Drive";
    pub const ATA: &str = "Drive.Ata";
    pub const NVME_CONTROLLER: &str = "NVMe.Controller";
    pub const PARTITION: &str = "Partition";
    pub const PARTITION_TABLE: &str = "PartitionTable";
    pub const FILESYSTEM: &str = "Filesystem";
    pub const ENCRYPTED: &str = "Encrypted";
    pub const SWAPSPACE: &str = "Swapspace";
    pub const LOOP: &str = "Loop";
}

/// The D-Bus object path UDisks2 uses for "no object".
pub const NO_OBJECT: &str = "/";

pub type Properties = BTreeMap<String, Value>;
pub type Interfaces = BTreeMap<String, Properties>;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ObjectMap {
    pub objects: BTreeMap<String, Interfaces>,
    #[serde(default)]
    pub capabilities: ManagerCapabilities,
    /// `NVMe.Controller.SmartGetAttributes` results keyed by drive object path.
    #[serde(default)]
    pub nvme_attributes: BTreeMap<String, Properties>,
}

/// Returns the short interface name when it is one the snapshot tracks.
pub fn tracked_interface(full_name: &str) -> Option<&'static str> {
    let short_name = full_name.strip_prefix(UDISKS_INTERFACE_PREFIX)?;
    TRACKED_INTERFACES
        .into_iter()
        .find(|tracked| *tracked == short_name)
}

impl ObjectMap {
    /// Paths of every object carrying `interface`, in path order.
    pub fn with_interface<'map>(
        &'map self,
        interface: &'map str,
    ) -> impl Iterator<Item = (&'map str, &'map Properties)> + 'map {
        self.objects
            .iter()
            .filter_map(move |(path, interfaces)| Some((path.as_str(), interfaces.get(interface)?)))
    }

    /// Applies `InterfacesAdded`; returns whether anything tracked changed.
    pub fn add_interfaces(&mut self, path: &str, added: Interfaces) -> bool {
        if added.is_empty() {
            return false;
        }
        self.objects
            .entry(path.to_owned())
            .or_default()
            .extend(added);
        true
    }

    /// Applies `InterfacesRemoved`; returns whether anything tracked changed.
    pub fn remove_interfaces<'name>(
        &mut self,
        path: &str,
        removed: impl IntoIterator<Item = &'name str>,
    ) -> bool {
        let Some(interfaces) = self.objects.get_mut(path) else {
            return false;
        };
        let removed_any = removed
            .into_iter()
            .filter(|name| interfaces.remove(*name).is_some())
            .count()
            > 0;
        if interfaces.is_empty() {
            self.objects.remove(path);
            self.nvme_attributes.remove(path);
        }
        removed_any
    }

    /// Applies `PropertiesChanged`; returns whether anything tracked changed.
    pub fn change_properties<'name>(
        &mut self,
        path: &str,
        interface: &str,
        changed: Properties,
        invalidated: impl IntoIterator<Item = &'name str>,
    ) -> bool {
        let Some(properties) = self
            .objects
            .get_mut(path)
            .and_then(|interfaces| interfaces.get_mut(interface))
        else {
            return false;
        };
        let invalidated_any = invalidated
            .into_iter()
            .filter(|name| properties.remove(*name).is_some())
            .count()
            > 0;
        let mut changed_any = invalidated_any;
        for (name, value) in changed {
            if properties.get(&name) != Some(&value) {
                properties.insert(name, value);
                changed_any = true;
            }
        }
        changed_any
    }
}

#[cfg(test)]
impl ObjectMap {
    pub fn interface(&self, path: &str, interface: &str) -> Option<&Properties> {
        self.objects.get(path)?.get(interface)
    }

    pub fn has_interface(&self, path: &str, interface: &str) -> bool {
        self.interface(path, interface).is_some()
    }

    pub fn property(&self, path: &str, interface: &str, property: &str) -> Option<&Value> {
        self.interface(path, interface)?.get(property)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block_with_size(size: u64) -> Interfaces {
        Interfaces::from([(
            interface::BLOCK.to_owned(),
            Properties::from([("Size".to_owned(), Value::Uint(size))]),
        )])
    }

    #[test]
    fn only_udisks_interfaces_the_snapshot_reads_are_tracked() {
        assert_eq!(
            tracked_interface("org.freedesktop.UDisks2.Drive.Ata"),
            Some(interface::ATA)
        );
        assert_eq!(tracked_interface("org.freedesktop.UDisks2.Job"), None);
        assert_eq!(tracked_interface("org.freedesktop.DBus.Properties"), None);
    }

    #[test]
    fn patches_report_whether_they_changed_anything() {
        let path = "/org/freedesktop/UDisks2/block_devices/sda";
        let mut map = ObjectMap::default();
        assert!(map.add_interfaces(path, block_with_size(1)));

        let same = Properties::from([("Size".to_owned(), Value::Uint(1))]);
        assert!(!map.change_properties(path, interface::BLOCK, same, []));
        let grown = Properties::from([("Size".to_owned(), Value::Uint(2))]);
        assert!(map.change_properties(path, interface::BLOCK, grown, []));
        assert_eq!(
            map.property(path, interface::BLOCK, "Size"),
            Some(&Value::Uint(2))
        );

        assert!(map.remove_interfaces(path, [interface::BLOCK]));
        assert!(map.objects.is_empty());
        assert!(!map.remove_interfaces(path, [interface::BLOCK]));
    }

    #[test]
    fn round_trips_through_json() {
        let mut map = ObjectMap::default();
        map.add_interfaces("/a", block_with_size(7));
        let json = serde_json::to_string(&map).unwrap();
        assert_eq!(serde_json::from_str::<ObjectMap>(&json).unwrap(), map);
    }
}
