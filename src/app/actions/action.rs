use crownos_disks::guard::Action;
use crownui::kit::icons;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiskAction {
    Mount,
    Unmount,
    Unlock,
    Lock,
    CheckRepair,
    Resize,
    Format,
    Rename,
    Delete,
    Erase,
    CreateImage,
    RestoreImage,
    Eject,
    CloseImage,
    NewPartition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    Toolbar,
    ImageMenu,
    MoreMenu,
}

impl DiskAction {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Mount => "Mount",
            Self::Unmount => "Unmount",
            Self::Unlock => "Unlock",
            Self::Lock => "Lock",
            Self::CheckRepair => "Check & Repair",
            Self::Resize => "Resize",
            Self::Format => "Format",
            Self::Rename => "Rename",
            Self::Delete => "Delete",
            Self::Erase => "Erase",
            Self::CreateImage => "Create Image",
            Self::RestoreImage => "Restore Image",
            Self::Eject => "Eject",
            Self::CloseImage => "Close Image",
            Self::NewPartition => "New Partition",
        }
    }

    pub const fn icon(self) -> &'static str {
        match self {
            Self::Mount => icons::HARD_DRIVE_DOWNLOAD,
            Self::Unmount | Self::Eject | Self::CloseImage => icons::ARROW_UP_FROM_LINE,
            Self::Unlock => icons::LOCK_OPEN,
            Self::Lock => icons::LOCK,
            Self::CheckRepair => icons::STETHOSCOPE,
            Self::Resize => icons::MOVE_HORIZONTAL,
            Self::Format => icons::ERASER,
            Self::Rename => icons::PENCIL,
            Self::Delete => icons::TRASH_2,
            Self::Erase => icons::ERASER,
            Self::CreateImage => icons::HARD_DRIVE_DOWNLOAD,
            Self::RestoreImage => icons::HARD_DRIVE_UPLOAD,
            Self::NewPartition => icons::PLUS,
        }
    }

    pub const fn placement(self) -> Placement {
        match self {
            Self::CreateImage | Self::RestoreImage => Placement::ImageMenu,
            Self::Rename | Self::Delete => Placement::MoreMenu,
            _ => Placement::Toolbar,
        }
    }

    pub const fn guard_action(self) -> Option<Action> {
        Some(match self {
            Self::Mount => Action::Mount,
            Self::Unmount => Action::Unmount,
            Self::Unlock => Action::Unlock,
            Self::Lock => Action::Lock,
            Self::CheckRepair => Action::Repair,
            Self::Resize => Action::Resize,
            Self::Format => Action::Format,
            Self::Rename => Action::Rename,
            Self::Delete => Action::DeletePartition,
            Self::Erase => Action::EraseDisk,
            Self::CreateImage => Action::CreateImage,
            Self::RestoreImage => Action::RestoreImage,
            Self::Eject | Self::CloseImage => Action::Eject,
            Self::NewPartition => Action::CreatePartition,
        })
    }
}
