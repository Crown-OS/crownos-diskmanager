use std::path::PathBuf;

use super::object_view::ObjectView;
use crate::model::{
    BlockId, ByteSize, Content, Encryption, FilesystemKind, LuksVersion, Unlocked, Volume,
};
use crate::object_map::{ObjectMap, interface};

/// Bounds LUKS-in-LUKS nesting so malformed data can never recurse forever.
const MAX_NESTING: u8 = 4;

const LUKS_ID_TYPE: &str = "crypto_LUKS";
const SWAP_ID_TYPE: &str = "swap";

mod usage {
    pub const FILESYSTEM: &str = "filesystem";
    pub const CRYPTO: &str = "crypto";
    pub const RAID: &str = "raid";
}

/// What a block device holds, following unlocked LUKS volumes to their cleartext content.
pub(super) fn resolve(map: &ObjectMap, block: ObjectView<'_>) -> Content {
    resolve_nested(map, block, MAX_NESTING)
}

fn resolve_nested(map: &ObjectMap, block: ObjectView<'_>, depth_left: u8) -> Content {
    let id_usage = block.text(interface::BLOCK, "IdUsage");
    let id_type = block.text(interface::BLOCK, "IdType");
    match (id_usage, id_type) {
        (_, SWAP_ID_TYPE) => Content::Swap {
            active: block.flag(interface::SWAPSPACE, "Active"),
        },
        (usage::FILESYSTEM, _) => Content::Filesystem(volume(block)),
        (usage::CRYPTO, LUKS_ID_TYPE) => luks(map, block, depth_left),
        _ if block.has(interface::ENCRYPTED) && id_type == LUKS_ID_TYPE => {
            luks(map, block, depth_left)
        }
        (usage::RAID, _) => Content::Member {
            kind: id_type.to_owned(),
        },
        ("", "") => Content::Empty,
        _ => Content::Unknown {
            id_type: id_type.to_owned(),
        },
    }
}

fn volume(block: ObjectView<'_>) -> Volume {
    Volume {
        filesystem: FilesystemKind::from_udisks(block.text(interface::BLOCK, "IdType")),
        label: block.text(interface::BLOCK, "IdLabel").to_owned(),
        uuid: block.text(interface::BLOCK, "IdUUID").to_owned(),
        mount_points: block
            .list(interface::FILESYSTEM, "MountPoints")
            .iter()
            .map(PathBuf::from)
            .collect(),
        usage: None,
    }
}

fn luks(map: &ObjectMap, block: ObjectView<'_>, depth_left: u8) -> Content {
    let version = match block.text(interface::BLOCK, "IdVersion") {
        "1" => LuksVersion::Luks1,
        _ => LuksVersion::Luks2,
    };
    let unlocked =
        cleartext(map, block)
            .zip(depth_left.checked_sub(1))
            .map(|(cleartext, depth_left)| {
                Box::new(Unlocked {
                    cleartext: BlockId::new(cleartext.path),
                    content: resolve_nested(map, cleartext, depth_left),
                })
            });
    Content::Encrypted(Encryption {
        version,
        header_size: ByteSize(block.uint(interface::ENCRYPTED, "MetadataSize")),
        unlocked,
    })
}

fn cleartext<'map>(map: &'map ObjectMap, block: ObjectView<'map>) -> Option<ObjectView<'map>> {
    block
        .object(interface::ENCRYPTED, "CleartextDevice")
        .and_then(|path| ObjectView::find(map, path))
        .or_else(|| {
            ObjectView::all(map).find(|candidate| {
                candidate.object(interface::BLOCK, "CryptoBackingDevice") == Some(block.path)
            })
        })
}
