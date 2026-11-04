use crownos_disks::model::DiskKind;
use crownui::kit::{icon, icons};
use crownui::prelude::{Semantic, Styled, View};

pub const fn disk_glyph(kind: DiskKind) -> &'static str {
    match kind {
        DiskKind::Nvme | DiskKind::Ssd | DiskKind::Hdd => icons::HARD_DRIVE,
        DiskKind::Usb => icons::USB,
        DiskKind::SdCard => icons::CARD_SIM,
        DiskKind::Optical => icons::DISC,
        DiskKind::Image => icons::DISC_3,
    }
}

pub fn disk_icon(kind: DiskKind, size: f32) -> impl View {
    icon(disk_glyph(kind))
        .size(size)
        .shrink(0.0)
        .label(kind.label())
}

pub fn lock_icon(size: f32) -> impl View {
    icon(icons::LOCK).size(size).shrink(0.0)
}
