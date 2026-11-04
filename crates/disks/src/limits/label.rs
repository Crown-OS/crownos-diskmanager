use super::LabelError;
use crate::model::FilesystemKind;

const FAT_FORBIDDEN: &str = "*?.,;:/\\|+=<>[]\"";
const EXFAT_FORBIDDEN: &str = "\"*/:<>?\\|";

#[derive(Debug, Clone, Copy)]
enum Measure {
    Bytes,
    Utf16Units,
}

#[derive(Debug, Clone, Copy)]
struct LabelRule {
    limit: usize,
    measure: Measure,
    forbidden: &'static str,
}

const fn rule(limit: usize, measure: Measure) -> LabelRule {
    LabelRule {
        limit,
        measure,
        forbidden: "",
    }
}

fn label_rule(filesystem: &FilesystemKind) -> Option<LabelRule> {
    match filesystem {
        FilesystemKind::Fat32 => Some(LabelRule {
            forbidden: FAT_FORBIDDEN,
            ..rule(11, Measure::Bytes)
        }),
        FilesystemKind::Exfat => Some(LabelRule {
            forbidden: EXFAT_FORBIDDEN,
            ..rule(15, Measure::Utf16Units)
        }),
        FilesystemKind::Ext4 | FilesystemKind::Ext3 | FilesystemKind::Ext2 => {
            Some(rule(16, Measure::Bytes))
        }
        FilesystemKind::Ntfs => Some(rule(32, Measure::Utf16Units)),
        FilesystemKind::Xfs => Some(rule(12, Measure::Bytes)),
        FilesystemKind::Btrfs => Some(rule(255, Measure::Bytes)),
        FilesystemKind::F2fs => Some(rule(512, Measure::Utf16Units)),
        FilesystemKind::Swap => Some(rule(15, Measure::Bytes)),
        FilesystemKind::Other(_) => None,
    }
}

pub(super) fn validate_label(filesystem: &FilesystemKind, label: &str) -> Result<(), LabelError> {
    if let Some(character) = label.chars().find(|character| character.is_control()) {
        return Err(LabelError::InvalidCharacter {
            filesystem: filesystem.short_name().to_owned(),
            character,
        });
    }
    let Some(rule) = label_rule(filesystem) else {
        return Ok(());
    };
    if let Some(character) = label
        .chars()
        .find(|character| rule.forbidden.contains(*character))
    {
        return Err(LabelError::InvalidCharacter {
            filesystem: filesystem.short_name().to_owned(),
            character,
        });
    }
    let length = match rule.measure {
        Measure::Bytes => label.len(),
        Measure::Utf16Units => label.encode_utf16().count(),
    };
    match length > rule.limit {
        true => Err(LabelError::TooLong {
            filesystem: filesystem.short_name().to_owned(),
            limit: rule.limit,
        }),
        false => Ok(()),
    }
}

/// Filesystems whose label tools refuse to touch a mounted volume.
pub(crate) fn relabel_requires_unmount(filesystem: &FilesystemKind) -> bool {
    matches!(
        filesystem,
        FilesystemKind::Ntfs | FilesystemKind::Exfat | FilesystemKind::F2fs
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn too_long(filesystem: &FilesystemKind, label: &str) -> bool {
        matches!(
            validate_label(filesystem, label),
            Err(LabelError::TooLong { .. })
        )
    }

    #[test]
    fn limits_follow_each_filesystem() {
        let cases = [
            (FilesystemKind::Fat32, 11),
            (FilesystemKind::Exfat, 15),
            (FilesystemKind::Ext4, 16),
            (FilesystemKind::Ext2, 16),
            (FilesystemKind::Ntfs, 32),
            (FilesystemKind::Xfs, 12),
            (FilesystemKind::Btrfs, 255),
            (FilesystemKind::F2fs, 512),
            (FilesystemKind::Swap, 15),
        ];
        cases.iter().for_each(|(filesystem, limit)| {
            assert!(
                validate_label(filesystem, &"A".repeat(*limit)).is_ok(),
                "{filesystem:?} at limit"
            );
            assert!(
                too_long(filesystem, &"A".repeat(limit + 1)),
                "{filesystem:?} over limit"
            );
        });
    }

    #[test]
    fn byte_limits_count_utf8_and_utf16_limits_count_units() {
        assert!(too_long(&FilesystemKind::Ext4, &"é".repeat(9)));
        assert!(validate_label(&FilesystemKind::Exfat, &"é".repeat(15)).is_ok());
        assert!(too_long(&FilesystemKind::Exfat, &"😀".repeat(8)));
    }

    #[test]
    fn fat_rejects_reserved_characters() {
        FAT_FORBIDDEN.chars().for_each(|character| {
            assert_eq!(
                validate_label(&FilesystemKind::Fat32, &format!("A{character}")),
                Err(LabelError::InvalidCharacter {
                    filesystem: "FAT32".to_owned(),
                    character
                })
            );
        });
        assert!(validate_label(&FilesystemKind::Fat32, "MY STICK").is_ok());
        assert!(validate_label(&FilesystemKind::Ext4, "home.backup").is_ok());
    }

    #[test]
    fn control_characters_and_empty_labels() {
        assert!(validate_label(&FilesystemKind::Ext4, "").is_ok());
        assert!(matches!(
            validate_label(&FilesystemKind::Other("udf".into()), "a\nb"),
            Err(LabelError::InvalidCharacter {
                character: '\n',
                ..
            })
        ));
        assert!(validate_label(&FilesystemKind::Other("udf".into()), &"x".repeat(300)).is_ok());
    }
}
