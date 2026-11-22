use std::collections::HashSet;

use crownos_disks::model::{BlockId, Disk, Inventory};

use crate::eligibility::automountable_volume;
use crate::util::fstab::FstabSources;

/// Decides what to mount as drives come and go. Each volume is offered once
/// per appearance, so one the user unmounted, or a mount that failed, stays
/// unmounted until the drive is plugged in again.
pub struct Automounter {
    fstab: FstabSources,
    seen: HashSet<BlockId>,
}

impl Automounter {
    pub fn new(fstab: FstabSources) -> Self {
        Self {
            fstab,
            seen: HashSet::new(),
        }
    }

    /// The blocks that appeared since the last inventory and should be mounted now.
    pub fn observe(&mut self, inventory: &Inventory) -> Vec<BlockId> {
        let present: HashSet<&BlockId> = inventory
            .disks
            .iter()
            .flat_map(Disk::partitions)
            .map(|partition| &partition.block)
            .collect();
        self.seen.retain(|block| present.contains(block));
        let (fstab, seen) = (&self.fstab, &mut self.seen);
        inventory
            .disks
            .iter()
            .flat_map(|disk| {
                disk.partitions().filter_map(move |partition| {
                    Some((partition, automountable_volume(disk, partition, fstab)?))
                })
            })
            .filter(|(partition, _)| seen.insert(partition.block.clone()))
            .filter(|(_, volume)| !volume.is_mounted())
            .map(|(partition, _)| partition.block.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use crownos_disks::fixtures;
    use crownos_disks::model::{Content, Region};

    use super::*;

    const ESP: &str = "/dev/nvme1n1p5";
    const WORKSPACE: &str = "/dev/nvme0n1p3";
    const SANDISK: &str = "/dev/sda1";
    const DATA: &str = "/dev/sdb1";
    const BACKUP: &str = "/dev/sdc1";
    const LOCKED: &str = "/dev/sdd1";
    const PHOTOS: &str = "/dev/sdd2";

    fn devices(inventory: &Inventory, blocks: &[BlockId]) -> Vec<String> {
        inventory
            .disks
            .iter()
            .flat_map(Disk::partitions)
            .filter(|partition| blocks.contains(&partition.block))
            .map(|partition| partition.device.clone())
            .collect()
    }

    fn observe(automounter: &mut Automounter, inventory: &Inventory) -> Vec<String> {
        let blocks = automounter.observe(inventory);
        devices(inventory, &blocks)
    }

    fn unmount_content(content: &mut Content) {
        match content {
            Content::Filesystem(volume) => volume.mount_points.clear(),
            Content::Encrypted(encryption) => {
                if let Some(unlocked) = encryption.unlocked.as_mut() {
                    unmount_content(&mut unlocked.content);
                }
            }
            _ => {}
        }
    }

    fn unmounted(mut inventory: Inventory) -> Inventory {
        let partitions = inventory.disks.iter_mut().flat_map(|disk| &mut disk.layout);
        for region in partitions {
            if let Region::Partition(partition) = region {
                unmount_content(&mut partition.content);
            }
        }
        inventory
    }

    fn automounter() -> Automounter {
        Automounter::new(FstabSources::default())
    }

    #[test]
    fn mounts_unmounted_data_volumes_once() {
        let inventory = fixtures::ata_failing();
        let mut automounter = automounter();
        assert_eq!(observe(&mut automounter, &inventory), [DATA, BACKUP]);
        assert!(observe(&mut automounter, &inventory).is_empty());
    }

    #[test]
    fn leaves_firmware_partitions_alone() {
        let mounted = observe(&mut automounter(), &unmounted(fixtures::desktop()));
        assert!(mounted.iter().any(|device| device == WORKSPACE));
        assert!(!mounted.iter().any(|device| device == ESP));
    }

    #[test]
    fn leaves_images_and_locked_volumes_alone() {
        let removable = observe(&mut automounter(), &unmounted(fixtures::removable()));
        assert!(
            removable
                .iter()
                .all(|device| !device.starts_with("/dev/loop"))
        );
        let encrypted = observe(&mut automounter(), &unmounted(fixtures::encrypted()));
        assert!(encrypted.iter().any(|device| device == PHOTOS));
        assert!(!encrypted.iter().any(|device| device == LOCKED));
    }

    #[test]
    fn a_volume_the_user_unmounted_stays_unmounted() {
        let mut automounter = automounter();
        assert!(observe(&mut automounter, &fixtures::removable()).is_empty());
        let after_unmount = unmounted(fixtures::removable());
        assert!(observe(&mut automounter, &after_unmount).is_empty());
    }

    #[test]
    fn plugging_a_drive_in_again_mounts_it_again() {
        let mut automounter = automounter();
        let plugged_in = unmounted(fixtures::removable());
        let unplugged = unmounted(fixtures::desktop());
        assert!(observe(&mut automounter, &plugged_in).contains(&SANDISK.to_owned()));
        observe(&mut automounter, &unplugged);
        assert!(observe(&mut automounter, &plugged_in).contains(&SANDISK.to_owned()));
    }

    #[test]
    fn leaves_fstab_volumes_to_the_administrator() {
        let inventory = fixtures::ata_failing();
        let data_uuid = inventory
            .disks
            .iter()
            .flat_map(Disk::partitions)
            .find(|partition| partition.device == DATA)
            .and_then(|partition| partition.content.volume())
            .map(|volume| volume.uuid.to_uppercase())
            .unwrap_or_default();
        let fstab = format!(
            "UUID={data_uuid} /data ext4 defaults 0 2\n{BACKUP} /backup ntfs3 noauto 0 0\n"
        );
        let mut automounter = Automounter::new(FstabSources::parse(&fstab));
        assert!(observe(&mut automounter, &inventory).is_empty());
    }
}
