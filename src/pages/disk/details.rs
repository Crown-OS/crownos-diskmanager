use crownos_disks::model::{Disk, TableKind};
use crownui::kit::{section, setting_row};
use crownui::prelude::{AnyView, Cx, View};

use super::header::connection_label;
use crate::components::value_text;

pub fn details(_cx: &mut Cx, disk: Disk) -> impl View + use<> {
    let table = disk.table.map(|table| match table {
        TableKind::Gpt => "GUID (GPT)".to_owned(),
        TableKind::Mbr => "Master Boot Record (MBR)".to_owned(),
    });
    let rotation = disk.rotation_rate.map(|rate| match rate {
        0 => "Solid state".to_owned(),
        rpm => format!("{rpm} rpm"),
    });
    let facts = [
        ("Model", Some(disk.model.clone())),
        ("Serial number", Some(disk.serial.clone())),
        ("Firmware revision", Some(disk.revision.clone())),
        ("Capacity", Some(disk.size.to_string())),
        (
            "Partition table",
            Some(table.unwrap_or_else(|| "None".to_owned())),
        ),
        ("Connection", Some(connection_label(&disk.connection))),
        ("Rotation rate", rotation),
        ("Device", Some(disk.device.clone())),
        (
            "Backing file",
            disk.backing_file
                .as_ref()
                .map(|path| path.display().to_string()),
        ),
    ];
    let rows: Vec<AnyView> = facts
        .into_iter()
        .filter_map(|(label, value)| {
            value
                .filter(|value| !value.is_empty())
                .map(|value| (label, value))
        })
        .map(|(label, value)| setting_row(label, value_text(value)).boxed())
        .collect();
    section("Details", rows)
}
