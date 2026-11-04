use zbus::Connection;
use zbus::OwnedFd;

const BLOCKED: &str = "sleep:shutdown:idle";
const APPLICATION: &str = "CrownOS Disk Manager";
const BLOCK_MODE: &str = "block";

#[zbus::proxy(
    interface = "org.freedesktop.login1.Manager",
    default_service = "org.freedesktop.login1",
    default_path = "/org/freedesktop/login1"
)]
trait LoginManager {
    fn inhibit(&self, what: &str, who: &str, why: &str, mode: &str) -> zbus::Result<OwnedFd>;
}

/// Keeps the system from sleeping, shutting down or idling until dropped.
#[derive(Debug)]
pub(crate) struct SleepInhibitor {
    _lock: OwnedFd,
}

pub(crate) async fn inhibit_sleep(
    connection: &Connection,
    reason: &str,
) -> zbus::Result<SleepInhibitor> {
    let manager = LoginManagerProxy::new(connection).await?;
    let lock = manager
        .inhibit(BLOCKED, APPLICATION, reason, BLOCK_MODE)
        .await?;
    Ok(SleepInhibitor { _lock: lock })
}
