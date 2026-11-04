use crate::model::{Capabilities, FilesystemKind, FormatSupport, ResizeModes};
use crate::object_map::ManagerCapabilities;
use crate::util::package::package_providing;

mod resize_flag {
    pub const OFFLINE_SHRINK: u64 = 1 << 1;
    pub const OFFLINE_GROW: u64 = 1 << 2;
    pub const ONLINE_SHRINK: u64 = 1 << 3;
    pub const ONLINE_GROW: u64 = 1 << 4;
}

const LUKS2: &str = "luks2";

pub(super) fn capabilities(manager: &ManagerCapabilities) -> Capabilities {
    Capabilities {
        formats: manager
            .can_format
            .iter()
            .map(|(filesystem, probe)| FormatSupport {
                filesystem: FilesystemKind::from_udisks(filesystem),
                missing_package: (!probe.available)
                    .then(|| package_providing(&probe.missing_tool).to_owned()),
            })
            .collect(),
        resize: manager
            .can_resize
            .iter()
            .filter(|(_, probe)| probe.available)
            .map(|(filesystem, probe)| {
                (
                    FilesystemKind::from_udisks(filesystem),
                    resize_modes(probe.modes),
                )
            })
            .collect(),
        encryption: manager.encryption_types.iter().any(|kind| kind == LUKS2),
    }
}

const fn resize_modes(flags: u64) -> ResizeModes {
    ResizeModes {
        offline_shrink: flags & resize_flag::OFFLINE_SHRINK != 0,
        offline_grow: flags & resize_flag::OFFLINE_GROW != 0,
        online_shrink: flags & resize_flag::ONLINE_SHRINK != 0,
        online_grow: flags & resize_flag::ONLINE_GROW != 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ext4_flags_from_this_host_decode() {
        let modes = resize_modes(22);
        assert_eq!(
            modes,
            ResizeModes {
                offline_shrink: true,
                offline_grow: true,
                online_shrink: false,
                online_grow: true
            }
        );
    }
}
