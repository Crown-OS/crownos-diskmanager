use crate::command::{Command, QuitImpact, Target};
use crate::guard::target::{Resolved, resolve};
use crate::guard::{Action, Fingerprint};
use crate::model::{DiskId, Inventory};

const UNKNOWN_SUBJECT: &str = "the disk";

/// Whether an operation needs its disk to itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Exclusivity {
    Shared,
    /// Changes data or layout: no other operation may run on the disk meanwhile, and the
    /// system is kept awake until it finishes.
    Exclusive,
}

/// What the service needs to know to run, guard and report one command.
#[derive(Debug, Clone)]
pub(super) struct OperationPlan {
    pub label: String,
    pub failure_title: String,
    pub subject: String,
    pub target: Target,
    pub disk: Option<DiskId>,
    pub action: Option<Action>,
    pub expected: Option<Fingerprint>,
    pub exclusivity: Exclusivity,
    pub cancelable: bool,
    pub quit_impact: QuitImpact,
}

struct Shape {
    target: Target,
    action: Option<Action>,
    verbs: (&'static str, &'static str),
    exclusivity: Exclusivity,
    cancelable: bool,
    quit_impact: QuitImpact,
    expected: Option<Fingerprint>,
}

const fn shape(
    target: Target,
    action: Option<Action>,
    verbs: (&'static str, &'static str),
) -> Shape {
    Shape {
        target,
        action,
        verbs,
        exclusivity: Exclusivity::Shared,
        cancelable: false,
        quit_impact: QuitImpact::Harmless,
        expected: None,
    }
}

impl Shape {
    fn exclusive(self, expected: Option<&Fingerprint>) -> Self {
        Self {
            exclusivity: Exclusivity::Exclusive,
            expected: expected.cloned(),
            ..self
        }
    }

    fn on_quit(self, quit_impact: QuitImpact) -> Self {
        Self {
            quit_impact,
            ..self
        }
    }

    fn cancelable(self) -> Self {
        Self {
            cancelable: true,
            ..self
        }
    }
}

/// Describes an operation command; `None` for control commands that run no operation.
pub(super) fn plan(inventory: &Inventory, command: &Command) -> Option<OperationPlan> {
    let block = |block: &crate::model::BlockId| Target::Block(block.clone());
    let disk = |disk: &DiskId| Target::Disk(disk.clone());
    let shape = match command {
        Command::Mount(id) => shape(block(id), Some(Action::Mount), ("Mounting", "mount")),
        Command::Unmount(id) => shape(block(id), Some(Action::Unmount), ("Unmounting", "unmount")),
        Command::Eject(id) => shape(disk(id), Some(Action::Eject), ("Ejecting", "eject")),
        Command::Rename { block: id, .. } => {
            shape(block(id), Some(Action::Rename), ("Renaming", "rename"))
        }
        Command::Unlock { block: id, .. } => {
            shape(block(id), Some(Action::Unlock), ("Unlocking", "unlock"))
        }
        Command::Lock(id) => shape(block(id), Some(Action::Lock), ("Locking", "lock")),
        Command::Check(id) => shape(block(id), Some(Action::Check), ("Checking", "check"))
            .exclusive(None)
            .cancelable()
            .on_quit(QuitImpact::FinishesInBackground),
        Command::Repair(id) => shape(block(id), Some(Action::Repair), ("Repairing", "repair"))
            .exclusive(None)
            .cancelable()
            .on_quit(QuitImpact::FinishesInBackground),
        Command::Format {
            block: id,
            expected,
            ..
        } => shape(block(id), Some(Action::Format), ("Formatting", "format"))
            .exclusive(Some(expected))
            .on_quit(QuitImpact::FinishesInBackground),
        Command::EraseDisk {
            disk: id, expected, ..
        } => shape(disk(id), Some(Action::EraseDisk), ("Erasing", "erase"))
            .exclusive(Some(expected))
            .on_quit(QuitImpact::StopsPartway),
        Command::CreatePartition {
            disk: id, expected, ..
        } => shape(
            disk(id),
            Some(Action::CreatePartition),
            ("Creating a partition on", "create a partition on"),
        )
        .exclusive(Some(expected))
        .on_quit(QuitImpact::FinishesInBackground),
        Command::DeletePartition {
            block: id,
            expected,
        } => shape(
            block(id),
            Some(Action::DeletePartition),
            ("Deleting", "delete"),
        )
        .exclusive(Some(expected))
        .on_quit(QuitImpact::FinishesInBackground),
        Command::Resize {
            block: id,
            expected,
            ..
        } => shape(block(id), Some(Action::Resize), ("Resizing", "resize"))
            .exclusive(Some(expected))
            .on_quit(QuitImpact::StopsPartway),
        Command::ResumeInterrupted(entry) => {
            let target = entry
                .block
                .as_ref()
                .map_or_else(|| disk(&entry.disk), block);
            shape(target, None, ("Finishing resizing", "finish resizing"))
                .exclusive(Some(&entry.fingerprint))
                .on_quit(QuitImpact::StopsPartway)
        }
        Command::CreateImage { source, .. } => shape(
            source.clone(),
            Some(Action::CreateImage),
            ("Creating an image of", "create an image of"),
        )
        .exclusive(None)
        .cancelable()
        .on_quit(QuitImpact::LosesImage),
        Command::RestoreImage {
            target, expected, ..
        } => shape(
            target.clone(),
            Some(Action::RestoreImage),
            ("Restoring an image to", "restore the image to"),
        )
        .exclusive(Some(expected))
        .cancelable()
        .on_quit(QuitImpact::LeavesTargetUnusable),
        Command::OpenImage(path) => shape(
            Target::Disk(DiskId::new(path.display().to_string())),
            None,
            ("Opening", "open"),
        ),
        Command::CloseImage(id) => shape(disk(id), None, ("Closing", "close")),
        Command::DismissInterrupted(_)
        | Command::RefreshHealth(_)
        | Command::RefreshUsage
        | Command::Cancel(_) => {
            return None;
        }
    };
    let subject = match command {
        Command::ResumeInterrupted(entry) => entry.target_name.clone(),
        Command::OpenImage(path) => path.file_name().map_or_else(
            || path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        ),
        _ => subject_name(inventory, &shape.target),
    };
    let disk = match command {
        Command::OpenImage(_) => None,
        Command::ResumeInterrupted(entry) => Some(entry.disk.clone()),
        _ => disk_of(inventory, &shape.target),
    };
    let (ongoing, infinitive) = shape.verbs;
    Some(OperationPlan {
        label: format!("{ongoing} {subject}"),
        failure_title: format!("Couldn't {infinitive} {subject}"),
        subject,
        target: shape.target,
        disk,
        action: shape.action,
        expected: shape.expected,
        exclusivity: shape.exclusivity,
        cancelable: shape.cancelable,
        quit_impact: shape.quit_impact,
    })
}

fn subject_name(inventory: &Inventory, target: &Target) -> String {
    match resolve(inventory, target) {
        Some(Resolved::Disk(disk)) => disk.name.clone(),
        Some(Resolved::Block(block)) => block.name(),
        None => UNKNOWN_SUBJECT.to_owned(),
    }
}

fn disk_of(inventory: &Inventory, target: &Target) -> Option<DiskId> {
    match (resolve(inventory, target), target) {
        (Some(Resolved::Disk(disk)), _) => Some(disk.id.clone()),
        (Some(Resolved::Block(block)), _) => Some(block.disk.id.clone()),
        (None, Target::Disk(id)) => Some(id.clone()),
        (None, Target::Block(_)) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::FormatSpec;
    use crate::guard::test_inventory::{self, PHOTOS_BLOCK, USB_DISK};
    use crate::model::{BlockId, FilesystemKind};

    #[test]
    fn destructive_commands_are_exclusive_and_carry_their_fingerprint() {
        let inventory = test_inventory::desktop_with_usb();
        let expected = crate::guard::fingerprint(&inventory, &Target::Disk(DiskId::new(USB_DISK)))
            .expect("usb");
        let format = Command::Format {
            block: BlockId::new(PHOTOS_BLOCK),
            spec: FormatSpec {
                filesystem: FilesystemKind::Exfat,
                label: String::new(),
                encryption: None,
            },
            expected: expected.clone(),
        };
        let plan = plan(&inventory, &format).expect("operation");
        assert_eq!(plan.label, "Formatting Photos");
        assert_eq!(plan.failure_title, "Couldn't format Photos");
        assert_eq!(plan.exclusivity, Exclusivity::Exclusive);
        assert_eq!(plan.expected, Some(expected));
        assert_eq!(plan.disk, Some(DiskId::new(USB_DISK)));
        assert_eq!(plan.action, Some(Action::Format));
    }

    #[test]
    fn everyday_and_control_commands() {
        let inventory = test_inventory::desktop_with_usb();
        let mount =
            plan(&inventory, &Command::Mount(BlockId::new(PHOTOS_BLOCK))).expect("operation");
        assert_eq!(mount.exclusivity, Exclusivity::Shared);
        assert!(mount.expected.is_none());
        assert!(plan(&inventory, &Command::RefreshUsage).is_none());
        assert!(plan(&inventory, &Command::Cancel(crate::command::OperationId(1))).is_none());
        let open =
            plan(&inventory, &Command::OpenImage("/home/user/os.iso".into())).expect("operation");
        assert_eq!(open.label, "Opening os.iso");
        assert!(open.disk.is_none());
    }
}
