use super::Fingerprint;
use super::target::{Resolved, resolve};
use crate::command::Target;
use crate::model::{Disk, Inventory};

pub(super) fn fingerprint(inventory: &Inventory, target: &Target) -> Option<Fingerprint> {
    let disk = match resolve(inventory, target)? {
        Resolved::Disk(disk) => disk,
        Resolved::Block(block) => block.disk,
    };
    Some(of_disk(disk))
}

pub(crate) fn of_disk(disk: &Disk) -> Fingerprint {
    Fingerprint {
        drive_serial: disk
            .backing_file
            .as_ref()
            .map_or_else(|| disk.serial.clone(), |path| path.display().to_string()),
        size: disk.size,
        table_uuid: disk.table_uuid.clone(),
        partitions: disk
            .partitions()
            .map(|partition| (partition.uuid.clone(), partition.offset))
            .collect(),
    }
}
