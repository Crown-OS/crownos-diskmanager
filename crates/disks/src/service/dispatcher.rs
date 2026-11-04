use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use super::execute::{execute, verify};
use super::plan::{Exclusivity, OperationPlan, plan};
use crate::client::Session;
use crate::command::{Command, Event, OperationId};
use crate::error::{FailureContext, StorageError, UserError};
use crate::guard::Refusal;
use crate::journal::{Journal, JournalEntry};
use crate::model::DiskId;
use crate::operations::{EventSink, OperationContext};
use crate::util::blocking::run_blocking;
use crate::util::inhibit::inhibit_sleep;

struct Running {
    id: OperationId,
    disk: Option<DiskId>,
    exclusivity: Exclusivity,
    context: OperationContext,
}

#[derive(Default, Clone)]
struct Registry(Arc<Mutex<Vec<Running>>>);

impl Registry {
    fn lock(&self) -> MutexGuard<'_, Vec<Running>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Registers the operation unless it would share its disk with an exclusive one.
    fn try_register(&self, running: Running) -> Result<(), Refusal> {
        let mut operations = self.lock();
        let conflicts = running.disk.as_ref().is_some_and(|disk| {
            operations.iter().any(|other| {
                other.disk.as_ref() == Some(disk)
                    && (other.exclusivity == Exclusivity::Exclusive
                        || running.exclusivity == Exclusivity::Exclusive)
            })
        });
        match conflicts {
            true => Err(Refusal::Busy),
            false => {
                operations.push(running);
                Ok(())
            }
        }
    }

    fn unregister(&self, id: OperationId) {
        self.lock().retain(|running| running.id != id);
    }

    fn cancel(&self, id: OperationId) {
        self.lock()
            .iter()
            .filter(|running| running.id == id)
            .for_each(|running| running.context.cancel());
    }
}

/// Turns commands into concurrently running operations and reports their lifecycle.
pub(super) struct Dispatcher {
    sink: EventSink,
    journal: Option<Journal>,
    registry: Registry,
    next_id: u64,
}

/// Whether an empty interrupted-operations list is worth reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EmptyList {
    Report,
    Skip,
}

impl Dispatcher {
    pub fn new(sink: EventSink, journal: Option<Journal>) -> Self {
        Self {
            sink,
            journal,
            registry: Registry::default(),
            next_id: 0,
        }
    }

    pub async fn announce_interrupted(&self, empty: EmptyList) {
        announce_interrupted(&self.sink, self.journal.as_ref(), empty).await;
    }

    pub fn handle(&mut self, session: Option<&Arc<Session>>, command: Command) {
        match command {
            Command::Cancel(id) => self.registry.cancel(id),
            Command::RefreshUsage => session.iter().for_each(|session| session.refresh_usage()),
            Command::RefreshHealth(disk) => {
                if let Some(session) = session.cloned() {
                    tokio::spawn(async move { drop(session.refresh_health(&disk).await) });
                }
            }
            Command::DismissInterrupted(entry) => self.dismiss(entry),
            command => self.start(session, command),
        }
    }

    fn dismiss(&self, entry: JournalEntry) {
        let sink = self.sink.clone();
        let journal = self.journal.clone();
        tokio::spawn(async move {
            if let Some(store) = journal.clone() {
                drop(run_blocking(move || Ok(store.finish(&entry)?)).await);
            }
            announce_interrupted(&sink, journal.as_ref(), EmptyList::Report).await;
        });
    }

    fn start(&mut self, session: Option<&Arc<Session>>, command: Command) {
        let inventory = session.map(|session| session.current()).unwrap_or_default();
        let Some(plan) = plan(&inventory, &command) else {
            return;
        };
        self.next_id += 1;
        let id = OperationId(self.next_id);
        (self.sink)(Event::OperationStarted {
            id,
            label: plan.label.clone(),
            target: plan.target.clone(),
            cancelable: plan.cancelable,
            quit_impact: plan.quit_impact,
        });
        let Some(session) = session.cloned() else {
            (self.sink)(Event::OperationFinished {
                id,
                result: Err(UserError::service_unavailable()),
            });
            return;
        };
        let context = OperationContext::new(id, self.sink.clone());
        let running = Running {
            id,
            disk: plan.disk.clone(),
            exclusivity: plan.exclusivity,
            context: context.clone(),
        };
        if let Err(refusal) = self.registry.try_register(running) {
            let error = StorageError::from(refusal).to_user_error(failure_context(&plan));
            (self.sink)(Event::OperationFinished {
                id,
                result: Err(error),
            });
            return;
        }
        let sink = self.sink.clone();
        let journal = self.journal.clone();
        let registry = self.registry.clone();
        tokio::spawn(async move {
            let outcome = run(&session, &context, journal.as_ref(), &plan, command).await;
            session.refresh_usage();
            registry.unregister(id);
            let result = outcome.map_err(|error| error.to_user_error(failure_context(&plan)));
            sink(Event::OperationFinished { id, result });
            if plan.exclusivity == Exclusivity::Exclusive {
                announce_interrupted(&sink, journal.as_ref(), EmptyList::Report).await;
            }
        });
    }
}

async fn run(
    session: &Session,
    context: &OperationContext,
    journal: Option<&Journal>,
    plan: &OperationPlan,
    command: Command,
) -> Result<crate::command::Completion, StorageError> {
    let _inhibitor = match plan.exclusivity {
        Exclusivity::Exclusive => inhibit_sleep(session.connection(), &plan.label).await.ok(),
        Exclusivity::Shared => None,
    };
    verify(&session.current(), plan)?;
    execute(session, context, journal, command).await
}

fn failure_context(plan: &OperationPlan) -> FailureContext<'_> {
    FailureContext {
        title: &plan.failure_title,
        subject: &plan.subject,
    }
}

async fn announce_interrupted(sink: &EventSink, journal: Option<&Journal>, empty: EmptyList) {
    let Some(journal) = journal.cloned() else {
        return;
    };
    let pending = run_blocking(move || Ok(journal.pending()?))
        .await
        .unwrap_or_default();
    if !pending.is_empty() || empty == EmptyList::Report {
        sink(Event::Interrupted(pending));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn running(id: u64, disk: &str, exclusivity: Exclusivity) -> Running {
        let sink: EventSink = Arc::new(|_| {});
        Running {
            id: OperationId(id),
            disk: Some(DiskId::new(disk)),
            exclusivity,
            context: OperationContext::new(OperationId(id), sink),
        }
    }

    #[test]
    fn one_exclusive_operation_per_disk() {
        let registry = Registry::default();
        assert!(
            registry
                .try_register(running(1, "/usb", Exclusivity::Exclusive))
                .is_ok()
        );
        assert_eq!(
            registry.try_register(running(2, "/usb", Exclusivity::Exclusive)),
            Err(Refusal::Busy)
        );
        assert_eq!(
            registry.try_register(running(3, "/usb", Exclusivity::Shared)),
            Err(Refusal::Busy)
        );
        assert!(
            registry
                .try_register(running(4, "/nvme", Exclusivity::Exclusive))
                .is_ok()
        );
        registry.unregister(OperationId(1));
        assert!(
            registry
                .try_register(running(5, "/usb", Exclusivity::Shared))
                .is_ok()
        );
        assert!(
            registry
                .try_register(running(6, "/usb", Exclusivity::Shared))
                .is_ok()
        );
        assert_eq!(
            registry.try_register(running(7, "/usb", Exclusivity::Exclusive)),
            Err(Refusal::Busy)
        );
    }

    #[test]
    fn cancel_reaches_the_running_operation() {
        let registry = Registry::default();
        let operation = running(1, "/usb", Exclusivity::Exclusive);
        let context = operation.context.clone();
        assert!(registry.try_register(operation).is_ok());
        registry.cancel(OperationId(1));
        assert!(context.is_cancelled());
    }
}
