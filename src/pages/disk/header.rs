use crownos_disks::model::Disk;
use crownui::prelude::{Cx, Styled, View, hstack, spacer, vstack};

use crate::components::{caption, disk_icon, health_pill, title};

const ICON_SIZE: f32 = 32.0;
const HEADER_GAP: f32 = 14.0;
const TEXT_GAP: f32 = 2.0;

pub fn header(_cx: &mut Cx, disk: Disk) -> impl View + use<> {
    let kind = disk.kind.label();
    let connection = connection_label(&disk.connection);
    let mut parts = vec![disk.size.to_string(), kind.to_owned()];
    if !connection.is_empty() && !kind.contains(connection.as_str()) {
        parts.push(connection);
    }
    let summary = parts.join(" · ");
    hstack((
        disk_icon(disk.kind, ICON_SIZE),
        vstack((title(disk.name.clone()), caption(summary)))
            .gap(TEXT_GAP)
            .min_w(0.0)
            .shrink(1.0),
        spacer(),
        health_pill(&disk.health),
    ))
    .gap(HEADER_GAP)
    .items_center()
}

pub fn connection_label(bus: &str) -> String {
    match bus {
        "usb" => "USB".to_owned(),
        "sdio" => "SD card reader".to_owned(),
        "ata" | "sata" => "SATA".to_owned(),
        "nvme" => "NVMe".to_owned(),
        "ieee1394" => "FireWire".to_owned(),
        "loop" => "Loop device".to_owned(),
        other => other.to_owned(),
    }
}
