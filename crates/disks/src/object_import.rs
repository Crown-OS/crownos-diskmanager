//! Reads UDisks2 over D-Bus into the pure [`ObjectMap`] (read-only calls only).

use std::collections::{BTreeMap, HashMap};

use zbus::fdo::ManagedObjects;
use zbus::names::OwnedInterfaceName;
use zbus::{Connection, OwnedVariant, Variant};

use crate::error::StorageError;
use crate::object_map::{
    FormatProbe, IGNORED_PROPERTIES, Interfaces, ManagerCapabilities, ObjectMap, Properties,
    ResizeProbe, Value, interface, tracked_interface,
};

pub(crate) const UDISKS_SERVICE: &str = "org.freedesktop.UDisks2";
const NVME_CONTROLLER_INTERFACE: &str = "org.freedesktop.UDisks2.NVMe.Controller";
const SMART_GET_ATTRIBUTES: &str = "SmartGetAttributes";
const BYTE_SIGNATURE: &str = "y";

/// Takes a full snapshot: managed objects, Manager capabilities and NVMe attributes.
pub(crate) async fn read_object_map(
    client: &udisks2::Client,
    connection: &Connection,
) -> Result<ObjectMap, StorageError> {
    let managed = client
        .object_manager()
        .get_managed_objects()
        .await
        .map_err(udisks2::Error::from)?;
    let mut map = ObjectMap {
        objects: objects_from_dbus(managed),
        ..ObjectMap::default()
    };
    map.capabilities = read_capabilities(client.manager()).await;
    map.nvme_attributes = read_all_nvme_attributes(connection, &map).await;
    Ok(map)
}

pub(crate) fn objects_from_dbus(managed: ManagedObjects) -> BTreeMap<String, Interfaces> {
    managed
        .into_iter()
        .map(|(path, interfaces)| (path.to_string(), interfaces_from_dbus(interfaces)))
        .filter(|(_, interfaces)| !interfaces.is_empty())
        .collect()
}

pub(crate) fn interfaces_from_dbus(
    interfaces: HashMap<OwnedInterfaceName, HashMap<String, OwnedVariant>>,
) -> Interfaces {
    interfaces
        .into_iter()
        .filter_map(|(name, properties)| {
            Some((
                tracked_interface(name.as_str())?.to_owned(),
                properties_from_dbus(properties),
            ))
        })
        .collect()
}

pub(crate) fn properties_from_dbus<V: std::ops::Deref<Target = Variant<'static>>>(
    properties: impl IntoIterator<Item = (String, V)>,
) -> Properties {
    properties
        .into_iter()
        .filter(|(name, _)| !IGNORED_PROPERTIES.contains(&name.as_str()))
        .filter_map(|(name, variant)| Some((name, value_from_variant(&variant)?)))
        .collect()
}

/// Converts the value shapes UDisks2 uses; anything else (dicts, structs, fds) is dropped.
pub(crate) fn value_from_variant(variant: &Variant<'_>) -> Option<Value> {
    Some(match variant {
        Variant::Bool(value) => Value::Bool(*value),
        Variant::U8(value) => Value::Uint((*value).into()),
        Variant::U16(value) => Value::Uint((*value).into()),
        Variant::U32(value) => Value::Uint((*value).into()),
        Variant::U64(value) => Value::Uint(*value),
        Variant::I16(value) => Value::Int((*value).into()),
        Variant::I32(value) => Value::Int((*value).into()),
        Variant::I64(value) => Value::Int(*value),
        Variant::F64(value) => Value::Double(*value),
        Variant::Str(value) => Value::Text(value.as_str().to_owned()),
        Variant::ObjectPath(value) => Value::Text(value.as_str().to_owned()),
        Variant::Variant(inner) => return value_from_variant(inner),
        Variant::Array(array) if array.element_signature() == BYTE_SIGNATURE => {
            Value::Text(byte_string(array.inner())?)
        }
        Variant::Array(array) => Value::List(
            array
                .inner()
                .iter()
                .map(text_element)
                .collect::<Option<_>>()?,
        ),
        _ => return None,
    })
}

fn text_element(variant: &Variant<'_>) -> Option<String> {
    match variant {
        Variant::Str(value) => Some(value.as_str().to_owned()),
        Variant::ObjectPath(value) => Some(value.as_str().to_owned()),
        Variant::Array(array) if array.element_signature() == BYTE_SIGNATURE => {
            byte_string(array.inner())
        }
        _ => None,
    }
}

/// A NUL-terminated `ay` byte string; non-UTF-8 bytes are replaced.
fn byte_string(bytes: &[Variant<'_>]) -> Option<String> {
    let bytes: Vec<u8> = bytes
        .iter()
        .map(|byte| match byte {
            Variant::U8(byte) => Some(*byte),
            _ => None,
        })
        .collect::<Option<_>>()?;
    let text = bytes.strip_suffix(&[0]).unwrap_or(&bytes);
    Some(String::from_utf8_lossy(text).into_owned())
}

async fn read_capabilities(manager: &udisks2::manager::ManagerProxy<'_>) -> ManagerCapabilities {
    let supported_filesystems = manager.supported_filesystems().await.unwrap_or_default();
    let encryption_types = manager
        .supported_encryption_types()
        .await
        .unwrap_or_default();
    let mut can_format = BTreeMap::new();
    let mut can_resize = BTreeMap::new();
    for filesystem in &supported_filesystems {
        if let Ok((available, missing_tool)) = manager.can_format(filesystem).await {
            can_format.insert(
                filesystem.clone(),
                FormatProbe {
                    available,
                    missing_tool,
                },
            );
        }
        if let Ok((available, modes, missing_tool)) = manager.can_resize(filesystem).await {
            can_resize.insert(
                filesystem.clone(),
                ResizeProbe {
                    available,
                    modes: modes.bits(),
                    missing_tool,
                },
            );
        }
    }
    ManagerCapabilities {
        supported_filesystems,
        encryption_types,
        can_format,
        can_resize,
    }
}

async fn read_all_nvme_attributes(
    connection: &Connection,
    map: &ObjectMap,
) -> BTreeMap<String, Properties> {
    let mut attributes = BTreeMap::new();
    for (drive, _) in map.with_interface(interface::NVME_CONTROLLER) {
        if let Ok(drive_attributes) = read_nvme_attributes(connection, drive).await {
            attributes.insert(drive.to_owned(), drive_attributes);
        }
    }
    attributes
}

/// `NVMe.Controller.SmartGetAttributes`, read generically so new keys never break decoding.
pub(crate) async fn read_nvme_attributes(
    connection: &Connection,
    drive: &str,
) -> Result<Properties, StorageError> {
    let no_options: HashMap<&str, Variant<'_>> = HashMap::new();
    let reply = connection
        .call_method(
            Some(UDISKS_SERVICE),
            drive,
            Some(NVME_CONTROLLER_INTERFACE),
            SMART_GET_ATTRIBUTES,
            &(no_options,),
        )
        .await?;
    let attributes: HashMap<String, OwnedVariant> = reply.body().deserialize()?;
    Ok(properties_from_dbus(attributes))
}
