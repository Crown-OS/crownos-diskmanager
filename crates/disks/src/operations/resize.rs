use std::cmp::Ordering;

use zbus::Variant;

use super::check::{Inspection, is_consistent};
use super::completion::completed;
use super::job;
use super::recorder::{Recorder, entry};
use super::teardown::{mount, unmount};
use super::udisks::{ENCRYPTED, FILESYSTEM, PARTITION, call, options};
use crate::client::Session;
use crate::command::{Completion, Secret};
use crate::error::StorageError;
use crate::guard::Refusal;
use crate::journal::{Journal, JournalEntry, JournalOperation};
use crate::limits::resize::{Direction, ResizeLayers, ResizeMode, layers, plan};
use crate::model::{BlockId, ByteSize};
use crate::operations::OperationContext;

const MAXIMUM: u64 = 0;
const RESIZING_PHASE: &str = "Resizing";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    Check,
    Filesystem,
    Encryption,
    Partition,
}

/// Shrinking goes from the inside out and growing from the outside in, so an interruption
/// always leaves every layer no larger than its container.
const SHRINK_STEPS: [Step; 4] = [
    Step::Check,
    Step::Filesystem,
    Step::Encryption,
    Step::Partition,
];
const GROW_STEPS: [Step; 3] = [Step::Partition, Step::Encryption, Step::Filesystem];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MountChange {
    Unchanged,
    Unmounted,
    Mounted,
}

struct ResizeJob<'a> {
    session: &'a Session,
    context: &'a OperationContext,
    layers: ResizeLayers<'a>,
    direction: Direction,
    mode: ResizeMode,
    target_size: ByteSize,
    partition_request: u64,
    passphrase: Option<&'a Secret>,
}

pub(crate) async fn resize(
    session: &Session,
    context: &OperationContext,
    journal: Option<&Journal>,
    block: &BlockId,
    size: ByteSize,
    passphrase: Option<&Secret>,
) -> Result<Completion, StorageError> {
    let inventory = session.current();
    let plan = plan(&inventory, block)?;
    let bounds = plan.bounds;
    if size < bounds.min || size > bounds.max {
        return Err(Refusal::NoRoom.into());
    }
    let direction = match size.cmp(&bounds.current) {
        Ordering::Less => Direction::Shrink,
        Ordering::Greater => Direction::Grow,
        Ordering::Equal => {
            return Ok(completed(format!(
                "{} already has that size.",
                plan.layers.target.name()
            )));
        }
    };
    let operation = match direction {
        Direction::Shrink => JournalOperation::Shrink { target_size: size },
        Direction::Grow => JournalOperation::Grow { target_size: size },
    };
    let target = plan.layers.target;
    let record = entry(
        operation,
        target.disk,
        Some(&target.partition.block),
        target.name(),
    );
    let partition_request = match direction == Direction::Grow && size == bounds.max {
        true => MAXIMUM,
        false => size.bytes(),
    };
    let job = ResizeJob::new(
        session,
        context,
        plan.layers,
        (direction, size, partition_request),
        passphrase,
    )?;
    job.run(Recorder::begin(journal, record).await?).await
}

/// Finishes a resize interrupted by a crash or power loss, continuing after its last completed step.
pub(crate) async fn resume(
    session: &Session,
    context: &OperationContext,
    journal: Option<&Journal>,
    interrupted: JournalEntry,
) -> Result<Completion, StorageError> {
    let (direction, size) = match interrupted.operation {
        JournalOperation::Shrink { target_size } => (Direction::Shrink, target_size),
        JournalOperation::Grow { target_size } => (Direction::Grow, target_size),
        JournalOperation::Format | JournalOperation::EraseDisk | JournalOperation::RestoreImage => {
            return Err(Refusal::NotApplicable.into());
        }
    };
    let block = interrupted
        .block
        .clone()
        .ok_or_else(|| StorageError::Gone(interrupted.target_name.clone()))?;
    let inventory = session.current();
    let layers = layers(&inventory, &block)?;
    let job = ResizeJob::new(
        session,
        context,
        layers,
        (direction, size, size.bytes()),
        None,
    )?;
    job.run(Recorder::resume(journal, interrupted)).await
}

impl<'a> ResizeJob<'a> {
    fn new(
        session: &'a Session,
        context: &'a OperationContext,
        layers: ResizeLayers<'a>,
        (direction, target_size, partition_request): (Direction, ByteSize, u64),
        passphrase: Option<&'a Secret>,
    ) -> Result<Self, StorageError> {
        let mode = layers.mode(direction).ok_or_else(|| Refusal::Unsupported {
            filesystem: layers.volume.filesystem.short_name().to_owned(),
        })?;
        Ok(Self {
            session,
            context,
            layers,
            direction,
            mode,
            target_size,
            partition_request,
            passphrase,
        })
    }

    async fn run(self, mut recorder: Recorder) -> Result<Completion, StorageError> {
        let filesystem_path = self.layers.target.filesystem_block().object_path();
        let change = self.prepare_mount(filesystem_path).await?;
        let outcome = self.run_steps(&mut recorder).await;
        self.restore_mount(filesystem_path, change).await;
        match outcome {
            Ok(()) => {
                recorder.finish().await?;
                Ok(completed(format!(
                    "{} was resized to {}.",
                    self.layers.target.name(),
                    self.target_size
                )))
            }
            Err(error) if recorder.completed_steps() == 0 => {
                recorder.finish().await?;
                Err(error)
            }
            Err(error) => Err(error),
        }
    }

    async fn run_steps(&self, recorder: &mut Recorder) -> Result<(), StorageError> {
        let steps: &[Step] = match self.direction {
            Direction::Shrink => &SHRINK_STEPS,
            Direction::Grow => &GROW_STEPS,
        };
        for step in steps.iter().skip(usize::from(recorder.completed_steps())) {
            self.context.ensure_not_cancelled()?;
            self.run_step(*step).await?;
            recorder.advance().await?;
        }
        Ok(())
    }

    async fn run_step(&self, step: Step) -> Result<(), StorageError> {
        let target = self.layers.target;
        let filesystem_path = target.filesystem_block().object_path();
        let partition_path = target.partition.block.object_path();
        let inner_size = match self.direction {
            Direction::Shrink => self.target_size.saturating_sub(self.layers.header).bytes(),
            Direction::Grow => MAXIMUM,
        };
        let objects = [filesystem_path, partition_path];
        match step {
            Step::Check if self.mode == ResizeMode::Offline => {
                match is_consistent(
                    self.session,
                    self.context,
                    filesystem_path,
                    Inspection::Check,
                )
                .await?
                {
                    true => Ok(()),
                    false => Err(StorageError::NeedsRepair {
                        name: target.name(),
                    }),
                }
            }
            Step::Check => Ok(()),
            Step::Filesystem => {
                let work = call::<_, ()>(
                    self.session,
                    filesystem_path,
                    FILESYSTEM,
                    "Resize",
                    (inner_size, options()),
                );
                job::follow(self.session, self.context, RESIZING_PHASE, &objects, work).await
            }
            Step::Encryption if target.encryption().is_none() => Ok(()),
            Step::Encryption => {
                let mut encryption_options = options();
                if let Some(passphrase) = self.passphrase {
                    encryption_options.insert("passphrase", Variant::from(passphrase.expose()));
                }
                let work = call::<_, ()>(
                    self.session,
                    partition_path,
                    ENCRYPTED,
                    "Resize",
                    (inner_size, encryption_options),
                );
                job::follow(self.session, self.context, RESIZING_PHASE, &objects, work).await
            }
            Step::Partition => {
                let work = call::<_, ()>(
                    self.session,
                    partition_path,
                    PARTITION,
                    "Resize",
                    (self.partition_request, options()),
                );
                job::follow(self.session, self.context, RESIZING_PHASE, &objects, work).await
            }
        }
    }

    /// Offline resizes need the filesystem unmounted; online-only ones (Btrfs) need it mounted.
    async fn prepare_mount(&self, path: &str) -> Result<MountChange, StorageError> {
        match (self.mode, self.layers.volume.is_mounted()) {
            (ResizeMode::Offline, true) => unmount(self.session, path)
                .await
                .map(|()| MountChange::Unmounted),
            (ResizeMode::Online, false) => mount(self.session, path)
                .await
                .map(|_| MountChange::Mounted),
            _ => Ok(MountChange::Unchanged),
        }
    }

    async fn restore_mount(&self, path: &str, change: MountChange) {
        let restored = match change {
            MountChange::Unchanged => Ok(()),
            MountChange::Unmounted => mount(self.session, path).await.map(drop),
            MountChange::Mounted => unmount(self.session, path).await,
        };
        drop(restored);
    }
}
