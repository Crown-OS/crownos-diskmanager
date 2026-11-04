//! UDisks2 signals → patched `ObjectMap` → debounced `Inventory`.

mod signal;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use signal::Change;
use tokio::sync::{mpsc, watch};
use tokio::time::{Instant, MissedTickBehavior};
use zbus::message::Type as MessageType;
use zbus::{Connection, MatchRule, MessageStream};

use crate::error::StorageError;
use crate::model::{Inventory, Usage};
use crate::object_import::{UDISKS_SERVICE, read_nvme_attributes};
use crate::object_map::{ObjectMap, Properties};
use crate::snapshot;
use crate::util::usage::filesystem_usage;

const UDISKS_NAMESPACE: &str = "/org/freedesktop/UDisks2";
const BUS_SERVICE: &str = "org.freedesktop.DBus";
const NAME_OWNER_CHANGED: &str = "NameOwnerChanged";
const REBUILD_DEBOUNCE: Duration = Duration::from_millis(50);
const USAGE_REFRESH_INTERVAL: Duration = Duration::from_secs(30);

pub(crate) enum WatcherRequest {
    RefreshUsage,
    NvmeAttributes {
        drive: String,
        attributes: Properties,
    },
}

/// Signal streams, opened before the initial `GetManagedObjects` so no change is missed.
pub(crate) struct Subscriptions {
    udisks: MessageStream,
    owner: MessageStream,
}

pub(crate) async fn subscribe(connection: &Connection) -> Result<Subscriptions, StorageError> {
    let udisks_rule = MatchRule::builder()
        .msg_type(MessageType::Signal)
        .sender(UDISKS_SERVICE)
        .path_namespace(UDISKS_NAMESPACE)
        .build()?;
    let owner_rule = MatchRule::builder()
        .msg_type(MessageType::Signal)
        .sender(BUS_SERVICE)
        .interface(BUS_SERVICE)
        .member(NAME_OWNER_CHANGED)
        .arg(0, UDISKS_SERVICE)
        .build()?;
    Ok(Subscriptions {
        udisks: MessageStream::for_match_rule(udisks_rule, connection, None).await?,
        owner: MessageStream::for_match_rule(owner_rule, connection, None).await?,
    })
}

pub(crate) struct Watcher {
    connection: Connection,
    map: ObjectMap,
    uid: u32,
    usage_by_mount_point: BTreeMap<PathBuf, Option<Usage>>,
    inventory: watch::Sender<Arc<Inventory>>,
    service_lost: watch::Sender<bool>,
}

impl Watcher {
    pub fn new(
        connection: Connection,
        map: ObjectMap,
        uid: u32,
        inventory: watch::Sender<Arc<Inventory>>,
        service_lost: watch::Sender<bool>,
    ) -> Self {
        Self {
            connection,
            map,
            uid,
            usage_by_mount_point: BTreeMap::new(),
            inventory,
            service_lost,
        }
    }

    /// Rebuilds the inventory and publishes it if it changed. Usage is measured
    /// again when asked to or when the set of mounted volumes changed.
    pub async fn publish(&mut self, remeasure_usage: bool) {
        let mut inventory = snapshot::build(&self.map, self.uid);
        let mount_points = snapshot::usage_mount_points(&inventory);
        if remeasure_usage || !mount_points.iter().eq(self.usage_by_mount_point.keys()) {
            self.usage_by_mount_point = measure_usage(mount_points).await;
        }
        snapshot::fill_usage(&mut inventory, |mount_point| {
            self.usage_by_mount_point
                .get(mount_point)
                .copied()
                .flatten()
        });
        self.inventory.send_if_modified(|current| {
            let changed = **current != inventory;
            if changed {
                *current = Arc::new(inventory);
            }
            changed
        });
    }

    /// Runs until the Session is dropped or UDisks2 goes away; in the latter
    /// case `service_lost` becomes `true`.
    pub async fn run(
        mut self,
        mut subscriptions: Subscriptions,
        mut requests: mpsc::UnboundedReceiver<WatcherRequest>,
    ) {
        let mut usage_timer = tokio::time::interval_at(
            Instant::now() + USAGE_REFRESH_INTERVAL,
            USAGE_REFRESH_INTERVAL,
        );
        usage_timer.set_missed_tick_behavior(MissedTickBehavior::Delay);
        let mut rebuild_at: Option<Instant> = None;
        let mut remeasure_usage = false;
        loop {
            let rebuild_due = async {
                match rebuild_at {
                    Some(deadline) => tokio::time::sleep_until(deadline).await,
                    None => std::future::pending().await,
                }
            };
            tokio::select! {
                message = subscriptions.udisks.next() => {
                    let Some(message) = message else { break };
                    let Ok(message) = message else { continue };
                    match signal::apply(&mut self.map, &message) {
                        Change::Nothing => continue,
                        Change::Inventory => {}
                        Change::NvmeSmart(drive) => self.reload_nvme_attributes(drive).await,
                    }
                    rebuild_at.get_or_insert_with(|| Instant::now() + REBUILD_DEBOUNCE);
                }
                message = subscriptions.owner.next() => {
                    if message.is_none_or(|message| message.is_ok_and(|message| owner_vanished(&message))) {
                        break;
                    }
                }
                request = requests.recv() => {
                    match request {
                        None => return,
                        Some(WatcherRequest::RefreshUsage) => remeasure_usage = true,
                        Some(WatcherRequest::NvmeAttributes { drive, attributes }) => {
                            self.map.nvme_attributes.insert(drive, attributes);
                        }
                    }
                    rebuild_at.get_or_insert_with(Instant::now);
                }
                () = rebuild_due => {
                    rebuild_at = None;
                    self.publish(std::mem::take(&mut remeasure_usage)).await;
                }
                _ = usage_timer.tick() => self.publish(true).await,
            }
        }
        self.service_lost.send_replace(true);
    }

    async fn reload_nvme_attributes(&mut self, drive: String) {
        if let Ok(attributes) = read_nvme_attributes(&self.connection, &drive).await {
            self.map.nvme_attributes.insert(drive, attributes);
        }
    }
}

/// `NameOwnerChanged(name, old_owner, new_owner)` with an empty new owner.
fn owner_vanished(message: &zbus::Message) -> bool {
    message
        .body()
        .deserialize::<(String, String, String)>()
        .is_ok_and(|(_, _, new_owner)| new_owner.is_empty())
}

async fn measure_usage(mount_points: Vec<PathBuf>) -> BTreeMap<PathBuf, Option<Usage>> {
    tokio::task::spawn_blocking(move || {
        mount_points
            .into_iter()
            .map(|mount_point| {
                let usage = filesystem_usage(&mount_point);
                (mount_point, usage)
            })
            .collect()
    })
    .await
    .unwrap_or_default()
}
