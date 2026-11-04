use std::collections::BTreeMap;
use std::sync::Arc;

use crownos_disks::command::{Completion, OperationId, Progress, QuitImpact, Target};
use crownos_disks::journal::JournalEntry;
use crownos_disks::model::{BlockId, ByteSize, DiskId, Inventory};
use crownos_disks::{Command, Event, UserError};
use crownui::prelude::{Cx, Runtime, Signal};
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Selection {
    #[default]
    Overview,
    Disk(DiskId),
    Partition {
        disk: DiskId,
        block: BlockId,
    },
    Free {
        disk: DiskId,
        offset: ByteSize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogRequest {
    EraseDisk(DiskId),
    NewPartition {
        disk: DiskId,
        offset: ByteSize,
        size: ByteSize,
    },
    Format(BlockId),
    Resize(BlockId),
    Rename(BlockId),
    Unlock(BlockId),
    Delete(BlockId),
    Repair(BlockId),
    CreateImage(Target),
    RestoreImage(Target),
    OpenImage,
    QuitWhileBusy,
}

#[derive(Debug, Clone)]
pub struct RunningOperation {
    pub label: String,
    pub target: Target,
    pub cancelable: bool,
    pub quit_impact: QuitImpact,
    pub phase: String,
    pub progress: Progress,
}

#[derive(Debug, Clone)]
pub struct FinishedOperation {
    pub id: OperationId,
    pub label: String,
    pub target: Option<Target>,
    pub result: Result<Completion, UserError>,
}

#[derive(Debug, Clone, Default)]
pub struct Operations {
    pub running: BTreeMap<OperationId, RunningOperation>,
    pub finished: Option<FinishedOperation>,
}

impl Operations {
    pub fn is_busy(&self, disk: &DiskId, inventory: &Inventory) -> bool {
        self.running
            .values()
            .any(|operation| match &operation.target {
                Target::Disk(id) => id == disk,
                Target::Block(block) => inventory
                    .partition(block)
                    .is_some_and(|(owner, _)| &owner.id == disk),
            })
    }

    /// The running operations that quitting would affect.
    pub fn interruptible(&self) -> impl Iterator<Item = (OperationId, &RunningOperation)> {
        self.running
            .iter()
            .filter(|(_, operation)| operation.quit_impact != QuitImpact::Harmless)
            .map(|(id, operation)| (*id, operation))
    }

    pub fn stopping_with_app(&self) -> impl Iterator<Item = OperationId> {
        self.interruptible()
            .filter(|(_, operation)| operation.quit_impact.stops_with_app())
            .map(|(id, _)| id)
    }
}

#[derive(Clone, Copy)]
pub struct Store {
    pub inventory: Signal<Arc<Inventory>>,
    pub selection: Signal<Selection>,
    pub dialog: Signal<Option<DialogRequest>>,
    pub operations: Signal<Operations>,
    pub interrupted: Signal<Vec<JournalEntry>>,
    pub service_error: Signal<Option<UserError>>,
    pub quitting: Signal<bool>,
    commands: Signal<Option<UnboundedSender<Command>>>,
}

impl Store {
    pub fn new(cx: &mut Cx) -> Self {
        Self {
            inventory: cx.signal(Arc::default()),
            selection: cx.signal(Selection::default()),
            dialog: cx.signal(None),
            operations: cx.signal(Operations::default()),
            interrupted: cx.signal(Vec::new()),
            service_error: cx.signal(None),
            quitting: cx.signal(false),
            commands: cx.signal(None),
        }
    }

    pub fn connect(self, runtime: &mut Runtime, commands: UnboundedSender<Command>) {
        self.commands.set(runtime, Some(commands));
    }

    pub fn send(self, runtime: &Runtime, command: Command) {
        self.commands.with_untracked(runtime, |commands| {
            if let Some(commands) = commands {
                _ = commands.send(command);
            }
        });
    }

    pub fn open_dialog(self, runtime: &mut Runtime, request: DialogRequest) {
        self.dialog.set(runtime, Some(request));
    }

    /// Cancels the operations that would die with the app and closes the window once they
    /// have stopped; operations running inside udisksd carry on.
    pub fn quit(self, cx: &mut Cx) {
        let stopping: Vec<OperationId> = self
            .operations
            .with_untracked(cx, |operations| operations.stopping_with_app().collect());
        for id in stopping {
            self.send(cx, Command::Cancel(id));
        }
        self.quitting.set(cx, true);
        self.close_if_quitting(cx);
    }

    fn close_if_quitting(self, cx: &mut Cx) {
        let stopped = self.operations.with_untracked(cx, |operations| {
            operations.stopping_with_app().next().is_none()
        });
        if stopped && self.quitting.with_untracked(cx, |quitting| *quitting) {
            cx.close_window();
        }
    }

    pub fn apply(self, cx: &mut Cx, event: Event) {
        match event {
            Event::Inventory(inventory) => {
                self.service_error.set(cx, None);
                self.inventory.set(cx, inventory);
            }
            Event::OperationStarted {
                id,
                label,
                target,
                cancelable,
                quit_impact,
            } => {
                self.operations.update(cx, |operations| {
                    let operation = RunningOperation {
                        label,
                        target,
                        cancelable,
                        quit_impact,
                        phase: String::new(),
                        progress: Progress::default(),
                    };
                    operations.running.insert(id, operation);
                });
            }
            Event::OperationProgress {
                id,
                phase,
                progress,
            } => {
                self.operations.update(cx, |operations| {
                    if let Some(operation) = operations.running.get_mut(&id) {
                        operation.phase = phase;
                        operation.progress = progress;
                    }
                });
            }
            Event::OperationFinished { id, result } => {
                self.operations.update(cx, |operations| {
                    let (label, target) = operations
                        .running
                        .remove(&id)
                        .map_or_else(Default::default, |operation| {
                            (operation.label, Some(operation.target))
                        });
                    operations.finished = Some(FinishedOperation {
                        id,
                        label,
                        target,
                        result,
                    });
                });
                self.close_if_quitting(cx);
            }
            Event::Interrupted(entries) => self.interrupted.set(cx, entries),
            Event::ServiceUnavailable(error) => self.service_error.set(cx, Some(error)),
        }
    }
}
