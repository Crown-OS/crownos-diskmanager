mod dispatcher;
mod execute;
mod plan;

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::watch;

use self::dispatcher::{Dispatcher, EmptyList};
use crate::client::Session;
use crate::command::{Command, Event};
use crate::error::UserError;
use crate::journal::Journal;
use crate::model::Inventory;
use crate::operations::EventSink;

const RECONNECT_INTERVAL: Duration = Duration::from_secs(2);

enum Ending {
    CommandsClosed,
    ServiceLost,
}

/// Runs the storage worker until the command channel closes.
///
/// Publishes the inventory as it changes, runs commands as operations (concurrently, one
/// exclusive operation per disk), reports interrupted operations from the journal, and
/// reconnects whenever the storage service comes back after going away.
pub async fn run(
    events: impl Fn(Event) + Send + Sync + 'static,
    mut commands: UnboundedReceiver<Command>,
) {
    let sink: EventSink = Arc::new(events);
    let mut dispatcher = Dispatcher::new(sink.clone(), Journal::locate());
    dispatcher.announce_interrupted(EmptyList::Skip).await;
    let mut unavailable_reported = false;
    loop {
        match Session::connect().await {
            Ok(session) => {
                match serve(&mut dispatcher, &Arc::new(session), &mut commands, &sink).await {
                    Ending::CommandsClosed => return,
                    Ending::ServiceLost => {
                        sink(Event::ServiceUnavailable(UserError::service_unavailable()));
                        unavailable_reported = true;
                    }
                }
            }
            Err(error) => {
                if !unavailable_reported {
                    let unavailable =
                        UserError::service_unavailable().with_details(error.to_string());
                    sink(Event::ServiceUnavailable(unavailable));
                    unavailable_reported = true;
                }
                if let Ending::CommandsClosed =
                    wait_to_reconnect(&mut dispatcher, &mut commands).await
                {
                    return;
                }
            }
        }
    }
}

async fn serve(
    dispatcher: &mut Dispatcher,
    session: &Arc<Session>,
    commands: &mut UnboundedReceiver<Command>,
    sink: &EventSink,
) -> Ending {
    let forwarder = tokio::spawn(forward_inventory(session.inventory.clone(), sink.clone()));
    let service_lost = session.service_lost();
    tokio::pin!(service_lost);
    let ending = loop {
        tokio::select! {
            () = &mut service_lost => break Ending::ServiceLost,
            command = commands.recv() => match command {
                Some(command) => dispatcher.handle(Some(session), command),
                None => break Ending::CommandsClosed,
            },
        }
    };
    forwarder.abort();
    ending
}

async fn wait_to_reconnect(
    dispatcher: &mut Dispatcher,
    commands: &mut UnboundedReceiver<Command>,
) -> Ending {
    let retry = tokio::time::sleep(RECONNECT_INTERVAL);
    tokio::pin!(retry);
    loop {
        tokio::select! {
            () = &mut retry => return Ending::ServiceLost,
            command = commands.recv() => match command {
                Some(command) => dispatcher.handle(None, command),
                None => return Ending::CommandsClosed,
            },
        }
    }
}

async fn forward_inventory(mut inventory: watch::Receiver<Arc<Inventory>>, sink: EventSink) {
    sink(Event::Inventory(inventory.borrow_and_update().clone()));
    while inventory.changed().await.is_ok() {
        sink(Event::Inventory(inventory.borrow_and_update().clone()));
    }
}
