use std::path::Path;

use super::plan::OperationPlan;
use crate::client::Session;
use crate::command::{Command, Completion, Target};
use crate::error::StorageError;
use crate::guard::target::{Resolved, resolve};
use crate::guard::{self, Availability, Refusal};
use crate::journal::{Journal, JournalOperation};
use crate::model::Inventory;
use crate::operations::check::Inspection;
use crate::operations::recorder::{Recorder, entry};
use crate::operations::{
    OperationContext, check, eject, encryption, format, image, mount, partition, rename, resize,
};

/// Confirms, right before the disk changes, that it is still the disk the user confirmed
/// and that the action is still allowed on it.
pub(super) fn verify(inventory: &Inventory, plan: &OperationPlan) -> Result<(), StorageError> {
    if let Some(expected) = &plan.expected {
        match guard::fingerprint(inventory, &plan.target) {
            None => return Err(StorageError::Gone(plan.subject.clone())),
            Some(current) if &current != expected => return Err(StorageError::Changed),
            Some(_) => {}
        }
    }
    match plan
        .action
        .map(|action| guard::availability(inventory, &plan.target, action))
    {
        Some(Availability::Unavailable(refusal)) => Err(refusal.into()),
        Some(Availability::Available) | None => Ok(()),
    }
}

pub(super) async fn execute(
    session: &Session,
    context: &OperationContext,
    journal: Option<&Journal>,
    command: Command,
) -> Result<Completion, StorageError> {
    match command {
        Command::Mount(block) => mount::mount(session, &block).await,
        Command::Unmount(block) => mount::unmount(session, &block).await,
        Command::Eject(disk) => eject::eject(session, &disk).await,
        Command::Rename { block, label } => rename::rename(session, &block, &label).await,
        Command::Unlock { block, passphrase } => {
            encryption::unlock(session, &block, &passphrase).await
        }
        Command::Lock(block) => encryption::lock(session, &block).await,
        Command::Check(block) => check::inspect(session, context, &block, Inspection::Check).await,
        Command::Repair(block) => {
            check::inspect(session, context, &block, Inspection::Repair).await
        }
        Command::Format { block, spec, .. } => {
            format::format(session, context, journal, &block, &spec).await
        }
        Command::EraseDisk {
            disk, scheme, spec, ..
        } => format::erase_disk(session, context, journal, &disk, scheme, &spec).await,
        Command::CreatePartition {
            disk,
            offset,
            size,
            name,
            spec,
            ..
        } => {
            partition::create(
                session,
                context,
                journal,
                &disk,
                (offset, size),
                &name,
                &spec,
            )
            .await
        }
        Command::DeletePartition { block, .. } => partition::delete(session, journal, &block).await,
        Command::Resize {
            block,
            size,
            passphrase,
            ..
        } => resize::resize(session, context, journal, &block, size, passphrase.as_ref()).await,
        Command::ResumeInterrupted(interrupted) => {
            resize::resume(session, context, journal, interrupted).await
        }
        Command::CreateImage {
            source,
            destination,
            compression,
        } => image::create(session, context, &source, &destination, compression).await,
        Command::RestoreImage {
            image,
            target,
            verify,
            ..
        } => restore_image(session, context, journal, &image, &target, verify).await,
        Command::OpenImage(path) => image::open(session, &path).await,
        Command::CloseImage(disk) => image::close(session, &disk).await,
        Command::DismissInterrupted(_)
        | Command::RefreshHealth(_)
        | Command::RefreshUsage
        | Command::Cancel(_) => Err(Refusal::NotApplicable.into()),
    }
}

/// Restores an image, leaving the journal entry behind unless it completes: a partial restore
/// leaves the target's contents incomplete.
async fn restore_image(
    session: &Session,
    context: &OperationContext,
    journal: Option<&Journal>,
    image_path: &Path,
    target: &Target,
    verify: bool,
) -> Result<Completion, StorageError> {
    let inventory = session.current();
    let record = match resolve(&inventory, target) {
        Some(Resolved::Disk(disk)) => entry(
            JournalOperation::RestoreImage,
            disk,
            None,
            disk.name.clone(),
        ),
        Some(Resolved::Block(block)) => entry(
            JournalOperation::RestoreImage,
            block.disk,
            Some(block.block()),
            block.name(),
        ),
        None => return Err(StorageError::Gone(String::from("The disk"))),
    };
    let recorder = Recorder::begin(journal, record).await?;
    let completion = image::restore(session, context, image_path, target, verify).await?;
    recorder.finish().await?;
    Ok(completion)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::FormatSpec;
    use crate::guard::test_inventory::{self, PHOTOS_BLOCK, ROOT_BLOCK, USB_DISK};
    use crate::model::{BlockId, DiskId, FilesystemKind};
    use crate::service::plan::plan;

    fn format(inventory: &Inventory, block: &str, disk: &str) -> Command {
        Command::Format {
            block: BlockId::new(block),
            spec: FormatSpec {
                filesystem: FilesystemKind::Ext4,
                label: String::new(),
                encryption: None,
            },
            expected: guard::fingerprint(inventory, &Target::Disk(DiskId::new(disk)))
                .expect("fingerprint"),
        }
    }

    #[test]
    fn a_swapped_disk_is_detected_before_formatting() {
        let confirmed = test_inventory::desktop_with_usb();
        let command = format(&confirmed, PHOTOS_BLOCK, USB_DISK);
        let operation = plan(&confirmed, &command).expect("operation");
        assert!(verify(&confirmed, &operation).is_ok());

        let mut replugged = test_inventory::desktop_with_usb();
        replugged.disks[1].serial = "different stick".to_owned();
        assert!(matches!(
            verify(&replugged, &operation),
            Err(StorageError::Changed)
        ));

        replugged.disks.pop();
        assert!(matches!(
            verify(&replugged, &operation),
            Err(StorageError::Gone(_))
        ));
    }

    #[test]
    fn guard_rules_are_enforced_again_by_the_service() {
        let inventory = test_inventory::desktop_with_usb();
        let command = format(&inventory, ROOT_BLOCK, test_inventory::NVME_DISK);
        let operation = plan(&inventory, &command).expect("operation");
        assert!(matches!(
            verify(&inventory, &operation),
            Err(StorageError::Refused(Refusal::SystemVolume { .. }))
        ));
    }
}
