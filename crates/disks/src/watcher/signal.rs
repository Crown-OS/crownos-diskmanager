use std::collections::HashMap;

use zbus::message::Message;
use zbus::names::OwnedInterfaceName;
use zbus::{OwnedObjectPath, OwnedVariant};

use crate::object_import::{interfaces_from_dbus, properties_from_dbus};
use crate::object_map::{ObjectMap, interface, tracked_interface};

const INTERFACES_ADDED: &str = "InterfacesAdded";
const INTERFACES_REMOVED: &str = "InterfacesRemoved";
const PROPERTIES_CHANGED: &str = "PropertiesChanged";
const SMART_UPDATED: &str = "SmartUpdated";

type AddedInterfaces = HashMap<OwnedInterfaceName, HashMap<String, OwnedVariant>>;

/// What a UDisks2 signal did to the map.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Change {
    Nothing,
    Inventory,
    /// The inventory changed and this NVMe drive's SMART data is new, so
    /// `SmartGetAttributes` must be read again.
    NvmeSmart(String),
}

/// Patches `map` with an ObjectManager or Properties signal.
pub(super) fn apply(map: &mut ObjectMap, message: &Message) -> Change {
    let header = message.header();
    let Some(member) = header.member() else {
        return Change::Nothing;
    };
    let body = message.body();
    match member.as_str() {
        INTERFACES_ADDED => match body.deserialize::<(OwnedObjectPath, AddedInterfaces)>() {
            Ok((path, added)) => {
                let added = interfaces_from_dbus(added);
                let adds_nvme = added.contains_key(interface::NVME_CONTROLLER);
                match map.add_interfaces(path.as_str(), added) {
                    true if adds_nvme => Change::NvmeSmart(path.to_string()),
                    true => Change::Inventory,
                    false => Change::Nothing,
                }
            }
            Err(_) => Change::Nothing,
        },
        INTERFACES_REMOVED => match body.deserialize::<(OwnedObjectPath, Vec<String>)>() {
            Ok((path, removed)) => {
                let removed = removed.iter().filter_map(|name| tracked_interface(name));
                changed_if(map.remove_interfaces(path.as_str(), removed))
            }
            Err(_) => Change::Nothing,
        },
        PROPERTIES_CHANGED => {
            let Some(path) = header.path() else {
                return Change::Nothing;
            };
            let Ok((name, changed, invalidated)) =
                body.deserialize::<(String, HashMap<String, OwnedVariant>, Vec<String>)>()
            else {
                return Change::Nothing;
            };
            let Some(short_name) = tracked_interface(&name) else {
                return Change::Nothing;
            };
            let smart_updated =
                short_name == interface::NVME_CONTROLLER && changed.contains_key(SMART_UPDATED);
            let invalidated = invalidated.iter().map(String::as_str);
            match map.change_properties(
                path.as_str(),
                short_name,
                properties_from_dbus(changed),
                invalidated,
            ) {
                true if smart_updated => Change::NvmeSmart(path.to_string()),
                changed => changed_if(changed),
            }
        }
        _ => Change::Nothing,
    }
}

const fn changed_if(changed: bool) -> Change {
    match changed {
        true => Change::Inventory,
        false => Change::Nothing,
    }
}

#[cfg(test)]
mod tests {
    use zbus::Variant;

    use super::*;
    use crate::object_map::Value;

    const OBJECT_MANAGER: &str = "org.freedesktop.DBus.ObjectManager";
    const PROPERTIES: &str = "org.freedesktop.DBus.Properties";
    const NVME_CONTROLLER_INTERFACE_NAME: &str = "org.freedesktop.UDisks2.NVMe.Controller";
    const STICK: &str = "/org/freedesktop/UDisks2/block_devices/sda1";

    fn added_stick() -> Message {
        let block = HashMap::from([
            ("Size", Variant::from(4096u64)),
            ("Device", Variant::from(b"/dev/sda1\0".to_vec())),
        ]);
        let job = HashMap::from([("Progress", Variant::from(0.5))]);
        let interfaces = HashMap::from([
            ("org.freedesktop.UDisks2.Block", block),
            ("org.freedesktop.UDisks2.Job", job),
        ]);
        Message::signal("/org/freedesktop/UDisks2", OBJECT_MANAGER, INTERFACES_ADDED)
            .build(&(OwnedObjectPath::try_from(STICK).unwrap(), interfaces))
            .unwrap()
    }

    #[test]
    fn interfaces_added_and_removed_patch_tracked_interfaces_only() {
        let mut map = ObjectMap::default();
        assert_eq!(apply(&mut map, &added_stick()), Change::Inventory);
        assert_eq!(
            map.property(STICK, interface::BLOCK, "Device"),
            Some(&Value::Text("/dev/sda1".into()))
        );
        assert!(!map.has_interface(STICK, "Job"));

        let removed = Message::signal(
            "/org/freedesktop/UDisks2",
            OBJECT_MANAGER,
            INTERFACES_REMOVED,
        )
        .build(&(
            OwnedObjectPath::try_from(STICK).unwrap(),
            vec!["org.freedesktop.UDisks2.Block"],
        ))
        .unwrap();
        assert_eq!(apply(&mut map, &removed), Change::Inventory);
        assert!(map.objects.is_empty());
    }

    #[test]
    fn mount_point_changes_are_applied() {
        let mut map = ObjectMap::default();
        apply(&mut map, &added_stick());
        map.add_interfaces(
            STICK,
            [(interface::FILESYSTEM.to_owned(), Default::default())].into(),
        );
        let mount_points = HashMap::from([(
            "MountPoints",
            Variant::from(vec![b"/run/media/user/STICK\0".to_vec()]),
        )]);
        let changed = Message::signal(STICK, PROPERTIES, PROPERTIES_CHANGED)
            .build(&(
                "org.freedesktop.UDisks2.Filesystem",
                mount_points,
                Vec::<&str>::new(),
            ))
            .unwrap();
        assert_eq!(apply(&mut map, &changed), Change::Inventory);
        assert_eq!(
            map.property(STICK, interface::FILESYSTEM, "MountPoints"),
            Some(&Value::List(vec!["/run/media/user/STICK".into()]))
        );
        assert_eq!(
            apply(&mut map, &changed),
            Change::Nothing,
            "repeated values change nothing"
        );
    }

    #[test]
    fn nvme_smart_updates_ask_for_fresh_attributes() {
        let drive = "/org/freedesktop/UDisks2/drives/nvme";
        let mut map = ObjectMap::default();
        map.add_interfaces(
            drive,
            [(interface::NVME_CONTROLLER.to_owned(), Default::default())].into(),
        );
        let updated = HashMap::from([("SmartUpdated", Variant::from(1_793_800_692u64))]);
        let changed = Message::signal(drive, PROPERTIES, PROPERTIES_CHANGED)
            .build(&(NVME_CONTROLLER_INTERFACE_NAME, updated, Vec::<&str>::new()))
            .unwrap();
        assert_eq!(
            apply(&mut map, &changed),
            Change::NvmeSmart(drive.to_owned())
        );
    }
}
