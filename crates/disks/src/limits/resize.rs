use super::ResizeBounds;
use crate::guard::target::{BlockTarget, Resolved, resolve_block};
use crate::guard::{DiskRole, Refusal, disk_role, in_system_use};
use crate::model::{
    BlockId, ByteSize, Content, FilesystemKind, Inventory, Region, ResizeModes, Volume,
};
use crate::util::alignment::align_up;

const SHRINK_MARGIN_PERCENT: u64 = 10;
const MIN_SHRINK_MARGIN: ByteSize = ByteSize::GIB;

/// How a filesystem layer gets resized in one direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResizeMode {
    /// Resize while mounted; an unmounted volume is mounted temporarily.
    Online,
    /// Resize while unmounted; a mounted volume is unmounted first.
    Offline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Direction {
    Shrink,
    Grow,
}

/// The layers of a resizable volume and how each direction resizes its filesystem.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ResizeLayers<'a> {
    pub target: BlockTarget<'a>,
    pub volume: &'a Volume,
    pub header: ByteSize,
    pub shrink: Option<ResizeMode>,
    pub grow: Option<ResizeMode>,
}

/// Resize layers together with the sizes the partition may take.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ResizePlan<'a> {
    pub layers: ResizeLayers<'a>,
    pub bounds: ResizeBounds,
}

impl ResizeLayers<'_> {
    pub fn mode(&self, direction: Direction) -> Option<ResizeMode> {
        match direction {
            Direction::Shrink => self.shrink,
            Direction::Grow => self.grow,
        }
    }
}

pub(crate) fn layers<'a>(
    inventory: &'a Inventory,
    block: &BlockId,
) -> Result<ResizeLayers<'a>, Refusal> {
    let Some(Resolved::Block(target)) = resolve_block(inventory, block) else {
        return Err(Refusal::NotApplicable);
    };
    if disk_role(target.disk) == DiskRole::LiveMedium {
        return Err(Refusal::LiveMedium);
    }
    if target.is_whole_disk() || target.is_extended_container() {
        return Err(Refusal::NotApplicable);
    }
    let volume = resizable_volume(&target)?;
    let system = in_system_use(&target.partition.content);
    let modes = inventory.resize_modes(&volume.filesystem);
    let shrink = mode(volume, modes, Direction::Shrink, system);
    let grow = mode(volume, modes, Direction::Grow, system);
    if shrink.is_none() && grow.is_none() {
        return Err(match system {
            true => Refusal::SystemVolume {
                name: target.name(),
            },
            false => Refusal::Unsupported {
                filesystem: volume.filesystem.short_name().to_owned(),
            },
        });
    }
    let header = target
        .encryption()
        .map_or(ByteSize(0), |encryption| encryption.header_size);
    Ok(ResizeLayers {
        target,
        volume,
        header,
        shrink,
        grow,
    })
}

pub(crate) fn plan<'a>(
    inventory: &'a Inventory,
    block: &BlockId,
) -> Result<ResizePlan<'a>, Refusal> {
    let layers = layers(inventory, block)?;
    let current = layers.target.partition.size;
    let bounds = ResizeBounds {
        min: layers
            .shrink
            .and_then(|_| minimum_size(layers.volume, layers.header, current))
            .unwrap_or(current),
        current,
        max: layers
            .grow
            .and_then(|_| following_free_end(&layers.target))
            .unwrap_or(current),
    };
    match bounds.min == bounds.max {
        true => Err(Refusal::NoRoom),
        false => Ok(ResizePlan { layers, bounds }),
    }
}

fn resizable_volume<'a>(target: &BlockTarget<'a>) -> Result<&'a Volume, Refusal> {
    if target.is_locked() {
        return Err(Refusal::Locked {
            name: target.name(),
        });
    }
    let Some(volume) = target.partition.content.volume() else {
        return Err(match target.content() {
            Content::Swap { .. } => Refusal::Unsupported {
                filesystem: FilesystemKind::Swap.short_name().to_owned(),
            },
            _ => Refusal::NotApplicable,
        });
    };
    match volume.filesystem {
        FilesystemKind::Exfat | FilesystemKind::Swap | FilesystemKind::Other(_) => {
            Err(Refusal::Unsupported {
                filesystem: volume.filesystem.short_name().to_owned(),
            })
        }
        _ => Ok(volume),
    }
}

fn mode(
    volume: &Volume,
    modes: ResizeModes,
    direction: Direction,
    system: bool,
) -> Option<ResizeMode> {
    let (online, offline) = match direction {
        Direction::Shrink if volume.filesystem == FilesystemKind::Xfs => (false, false),
        Direction::Shrink => (modes.online_shrink, modes.offline_shrink),
        Direction::Grow => (modes.online_grow, modes.offline_grow),
    };
    match (volume.is_mounted(), online, offline) {
        (true, true, _) => Some(ResizeMode::Online),
        (true, false, true) if !system => Some(ResizeMode::Offline),
        (false, _, true) => Some(ResizeMode::Offline),
        (false, true, false) => Some(ResizeMode::Online),
        _ => None,
    }
}

fn minimum_size(volume: &Volume, header: ByteSize, current: ByteSize) -> Option<ByteSize> {
    let used = volume.usage?.used.bytes();
    let margin = (used / 100 * SHRINK_MARGIN_PERCENT).max(MIN_SHRINK_MARGIN.bytes());
    let minimum = align_up(
        used.saturating_add(margin).saturating_add(header.bytes()),
        ByteSize::MIB.bytes(),
    );
    Some(ByteSize(minimum.min(current.bytes())))
}

fn following_free_end(target: &BlockTarget<'_>) -> Option<ByteSize> {
    let layout = &target.disk.layout;
    let position = layout
        .iter()
        .position(|region| matches!(region, Region::Partition(partition) if partition.block == target.partition.block))?;
    match layout.get(position + 1)? {
        Region::Free { offset, size } => Some(ByteSize(
            offset.bytes() + size.bytes() - target.partition.offset.bytes(),
        )),
        Region::Partition(_) => None,
    }
}
