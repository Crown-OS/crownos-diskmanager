use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, watch};
use zbus::{Connection, Variant};

use crate::error::StorageError;
use crate::model::{Disk, DiskId, DiskKind, Health, Inventory};
use crate::object_import::{UDISKS_SERVICE, read_nvme_attributes, read_object_map};
use crate::util::throttle::Throttle;
use crate::watcher::{self, Watcher, WatcherRequest};

const HEALTH_REFRESH_INTERVAL: Duration = Duration::from_secs(5 * 60);
const SMART_UPDATE: &str = "SmartUpdate";
const ATA_INTERFACE: &str = "org.freedesktop.UDisks2.Drive.Ata";
const NVME_CONTROLLER_INTERFACE: &str = "org.freedesktop.UDisks2.NVMe.Controller";

/// A live connection to UDisks2 with a self-updating inventory.
///
/// A background task (spawned on the current tokio runtime) keeps `inventory`
/// current from UDisks2 signals and refreshes usage every 30 s; it stops
/// when the session is dropped or the service goes away.
pub(crate) struct Session {
    pub client: udisks2::Client,
    pub inventory: watch::Receiver<Arc<Inventory>>,
    requests: mpsc::UnboundedSender<WatcherRequest>,
    service_lost: watch::Receiver<bool>,
    health_refresh: Throttle<DiskId>,
}

impl Session {
    /// Connects to the system bus, reads the full UDisks2 state and starts watching it.
    pub async fn connect() -> Result<Self, StorageError> {
        let connection = Connection::system().await?;
        let client = udisks2::Client::new_for_connection(connection.clone()).await?;
        let subscriptions = watcher::subscribe(&connection).await?;
        let map = read_object_map(&client, &connection).await?;

        let (inventory_sender, inventory) = watch::channel(Arc::default());
        let (service_lost_sender, service_lost) = watch::channel(false);
        let (requests, request_receiver) = mpsc::unbounded_channel();
        let uid = rustix::process::getuid().as_raw();
        let mut watcher = Watcher::new(connection, map, uid, inventory_sender, service_lost_sender);
        watcher.publish(true).await;
        tokio::spawn(watcher.run(subscriptions, request_receiver));

        Ok(Self {
            client,
            inventory,
            requests,
            service_lost,
            health_refresh: Throttle::new(HEALTH_REFRESH_INTERVAL),
        })
    }

    pub fn current(&self) -> Arc<Inventory> {
        self.inventory.borrow().clone()
    }

    pub fn connection(&self) -> &Connection {
        self.client.object_manager().inner().connection()
    }

    /// Measures filesystem usage again (after an operation or on request).
    pub fn refresh_usage(&self) {
        let _ = self.requests.send(WatcherRequest::RefreshUsage);
    }

    /// Asks the drive to refresh SMART data without waking a sleeping disk,
    /// at most once per 5 minutes per drive; the inventory updates on its
    /// own. Drives without SMART (USB bridges, SD cards, images) are skipped.
    pub async fn refresh_health(&self, disk: &DiskId) -> Result<(), StorageError> {
        let inventory = self.current();
        let Some(disk) = inventory.disk(disk) else {
            return Ok(());
        };
        let Some(interface) = smart_interface(disk) else {
            return Ok(());
        };
        if !self.health_refresh.try_begin(&disk.id) {
            return Ok(());
        }
        let options = HashMap::from([
            ("nowakeup", Variant::from(true)),
            ("auth.no_user_interaction", Variant::from(true)),
        ]);
        let drive = disk.id.object_path();
        self.connection()
            .call_method(
                Some(UDISKS_SERVICE),
                drive,
                Some(interface),
                SMART_UPDATE,
                &(options,),
            )
            .await?;
        if interface == NVME_CONTROLLER_INTERFACE {
            let attributes = read_nvme_attributes(self.connection(), drive).await?;
            let _ = self.requests.send(WatcherRequest::NvmeAttributes {
                drive: drive.to_owned(),
                attributes,
            });
        }
        Ok(())
    }

    /// Resolves once UDisks2 has gone away (or the watcher stopped); the
    /// caller should report the service as unavailable and offer to reconnect.
    pub fn service_lost(&self) -> impl Future<Output = ()> + Send + 'static {
        let mut service_lost = self.service_lost.clone();
        async move {
            let _ = service_lost.wait_for(|lost| *lost).await;
        }
    }
}

fn smart_interface(disk: &Disk) -> Option<&'static str> {
    match (disk.kind, &disk.health) {
        (DiskKind::Nvme, _) => Some(NVME_CONTROLLER_INTERFACE),
        (DiskKind::Ssd | DiskKind::Hdd, _) | (_, Health::Reported(_)) => Some(ATA_INTERFACE),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "reads the live UDisks2 daemon (read-only calls only)"]
    async fn connects_and_publishes_the_live_inventory() {
        let session = Session::connect().await.unwrap();
        let inventory = session.current();
        assert!(!inventory.disks.is_empty());
        let mounted_root = inventory
            .disks
            .iter()
            .flat_map(Disk::partitions)
            .filter_map(|partition| partition.content.volume())
            .find(|volume| {
                volume
                    .mount_points
                    .iter()
                    .any(|mount_point| mount_point.as_os_str() == "/")
            });
        assert!(mounted_root.is_some_and(|volume| volume.usage.is_some()));

        session.refresh_usage();
        tokio::time::sleep(Duration::from_millis(200)).await;
        let lost = tokio::time::timeout(Duration::from_millis(50), session.service_lost()).await;
        assert!(lost.is_err(), "UDisks2 is still running");
        let threads: Vec<String> = std::fs::read_dir("/proc/self/task")
            .unwrap()
            .filter_map(|task| std::fs::read_to_string(task.ok()?.path().join("comm")).ok())
            .collect();
        assert!(
            threads.iter().all(|name| !name.starts_with("zruntime")),
            "zbus runs on tokio: {threads:?}"
        );
    }
}
