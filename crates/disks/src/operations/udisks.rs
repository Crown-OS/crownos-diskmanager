use std::collections::HashMap;
use std::time::Duration;

use serde::Serialize;
use serde::de::DeserializeOwned;
use zbus::Variant;
use zbus::zvariant::{DynamicType, Type};

use crate::client::Session;
use crate::error::StorageError;

pub(crate) const BLOCK: &str = "org.freedesktop.UDisks2.Block";
pub(crate) const DRIVE: &str = "org.freedesktop.UDisks2.Drive";
pub(crate) const ENCRYPTED: &str = "org.freedesktop.UDisks2.Encrypted";
pub(crate) const FILESYSTEM: &str = "org.freedesktop.UDisks2.Filesystem";
pub(crate) const JOB: &str = "org.freedesktop.UDisks2.Job";
pub(crate) const PARTITION: &str = "org.freedesktop.UDisks2.Partition";
pub(crate) const PARTITION_TABLE: &str = "org.freedesktop.UDisks2.PartitionTable";
pub(crate) const SWAPSPACE: &str = "org.freedesktop.UDisks2.Swapspace";

const SERVICE: &str = "org.freedesktop.UDisks2";
const INTERFACE_WAIT_INTERVAL: Duration = Duration::from_millis(200);
const INTERFACE_WAIT_ATTEMPTS: u32 = 25;

/// Method options for UDisks2 calls, starting with the standard authorization option.
pub(crate) type Options<'a> = HashMap<&'static str, Variant<'a>>;

/// Whether polkit may show an authentication prompt for a call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Interaction {
    Allowed,
    Forbidden,
}

/// The app lets polkit prompt; tests never wait for a prompt nobody answers.
const INTERACTION: Interaction = match cfg!(test) {
    true => Interaction::Forbidden,
    false => Interaction::Allowed,
};

/// Options every operation sends, starting with the authorization option.
pub(crate) fn options<'a>() -> Options<'a> {
    HashMap::from([(
        "auth.no_user_interaction",
        Variant::from(INTERACTION == Interaction::Forbidden),
    )])
}

/// Calls `method` on a UDisks2 object, keeping the daemon's error text for the user's details.
pub(crate) async fn call<Body, Reply>(
    session: &Session,
    path: &str,
    interface: &'static str,
    method: &'static str,
    body: Body,
) -> Result<Reply, StorageError>
where
    Body: Serialize + DynamicType,
    Reply: DeserializeOwned + Type,
{
    let connection = session.client.object_manager().inner().connection();
    let reply = connection
        .call_method(Some(SERVICE), path, Some(interface), method, &body)
        .await?;
    Ok(reply.body().deserialize::<Reply>()?)
}

/// Like [`call`], but waits for `interface` to appear on a freshly created object first.
pub(crate) async fn call_when_ready<Body, Reply>(
    session: &Session,
    path: &str,
    interface: &'static str,
    method: &'static str,
    body: &Body,
) -> Result<Reply, StorageError>
where
    Body: Serialize + Type,
    Reply: DeserializeOwned + Type,
{
    let mut attempts = 0;
    loop {
        match call(session, path, interface, method, body).await {
            Err(error) if attempts < INTERFACE_WAIT_ATTEMPTS && is_not_ready(&error) => {
                attempts += 1;
                tokio::time::sleep(INTERFACE_WAIT_INTERVAL).await;
            }
            result => return result,
        }
    }
}

fn is_not_ready(error: &StorageError) -> bool {
    use zbus::fdo::Error as Fdo;
    match error {
        StorageError::Dbus(zbus::Error::FDO(fdo)) => matches!(
            **fdo,
            Fdo::UnknownMethod(_) | Fdo::UnknownInterface(_) | Fdo::UnknownObject(_)
        ),
        _ => false,
    }
}
