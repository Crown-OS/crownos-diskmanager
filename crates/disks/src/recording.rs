//! Recording the live UDisks2 state as fixture JSON, and replaying it.

use crate::error::StorageError;
use crate::model::Inventory;
use crate::object_import::read_object_map;
use crate::object_map::ObjectMap;
use crate::snapshot;

/// Reads the live system over D-Bus (read-only calls only) and returns the
/// `ObjectMap` as pretty-printed JSON, the format of `tests/fixtures/*.json`.
pub async fn record_system() -> Result<String, StorageError> {
    let connection = zbus::Connection::system().await?;
    let client = udisks2::Client::new_for_connection(connection.clone()).await?;
    let map = read_object_map(&client, &connection).await?;
    serde_json::to_string_pretty(&map).map_err(|error| StorageError::Io(error.into()))
}

/// Builds an inventory from recorded fixture JSON, as if `uid` were logged in.
pub fn inventory_from_json(json: &str, uid: u32) -> Result<Inventory, serde_json::Error> {
    serde_json::from_str::<ObjectMap>(json).map(|map| snapshot::build(&map, uid))
}
