use zbus::Variant;

use super::udisks::{Options, options};
use crate::command::FormatSpec;
use crate::error::StorageError;
use crate::guard::Refusal;
use crate::model::{FilesystemKind, Inventory};

const ENCRYPTION_TYPE: &str = "luks2";

/// `Block.Format` options for `spec`: always tears down what uses the block, checks the
/// parameters with a dry run first and updates the partition type to match.
pub(crate) fn format_options(spec: &FormatSpec) -> Options<'_> {
    let mut format = options();
    format.insert("tear-down", Variant::from(true));
    format.insert("dry-run-first", Variant::from(true));
    format.insert("update-partition-type", Variant::from(true));
    if !spec.label.is_empty() {
        format.insert("label", Variant::from(spec.label.as_str()));
    }
    if spec.filesystem.is_linux_native() {
        format.insert("take-ownership", Variant::from(true));
    }
    if let Some(passphrase) = &spec.encryption {
        format.insert("encrypt.passphrase", Variant::from(passphrase.expose()));
        format.insert("encrypt.type", Variant::from(ENCRYPTION_TYPE));
    }
    format
}

/// Refuses filesystems whose tools are missing, before anything on the disk changes.
pub(crate) fn ensure_formattable(
    inventory: &Inventory,
    spec: &FormatSpec,
) -> Result<(), StorageError> {
    let capabilities = &inventory.capabilities;
    if spec.encryption.is_some() && !capabilities.encryption {
        return Err(Refusal::Unsupported {
            filesystem: "This system".to_owned(),
        }
        .into());
    }
    let missing = capabilities
        .formats
        .iter()
        .find(|support| support.filesystem == spec.filesystem)
        .and_then(|support| support.missing_package.as_deref());
    match (missing, &spec.filesystem) {
        (_, FilesystemKind::Other(name)) => Err(Refusal::Unsupported {
            filesystem: name.to_string(),
        }
        .into()),
        (Some(tool), filesystem) => Err(Refusal::MissingTool {
            filesystem: filesystem.short_name().to_owned(),
            tool: tool.to_owned(),
        }
        .into()),
        (None, _) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Secret;
    use crate::guard::test_inventory;

    fn spec(filesystem: FilesystemKind, encryption: Option<Secret>) -> FormatSpec {
        FormatSpec {
            filesystem,
            label: "Data".to_owned(),
            encryption,
        }
    }

    #[test]
    fn formats_safely_and_encrypts_with_luks2() {
        let encrypted = spec(
            FilesystemKind::Ext4,
            Some(Secret::new("hunter2".to_owned())),
        );
        let format = format_options(&encrypted);
        [
            ("tear-down", true),
            ("dry-run-first", true),
            ("update-partition-type", true),
            ("take-ownership", true),
        ]
        .iter()
        .for_each(|(name, expected)| {
            assert_eq!(format.get(name), Some(&Variant::from(*expected)), "{name}")
        });
        assert_eq!(
            format.get("auth.no_user_interaction"),
            Some(&Variant::from(cfg!(test)))
        );
        assert_eq!(format.get("encrypt.type"), Some(&Variant::from("luks2")));
        assert_eq!(
            format.get("encrypt.passphrase"),
            Some(&Variant::from("hunter2"))
        );
        assert_eq!(format.get("label"), Some(&Variant::from("Data")));

        let exfat = spec(FilesystemKind::Exfat, None);
        let plain = format_options(&exfat);
        assert!(!plain.contains_key("encrypt.passphrase"));
        assert!(!plain.contains_key("take-ownership"));
    }

    #[test]
    fn missing_tools_are_refused() {
        let inventory = test_inventory::desktop_with_usb();
        assert!(ensure_formattable(&inventory, &spec(FilesystemKind::Ext4, None)).is_ok());
        assert!(matches!(
            ensure_formattable(&inventory, &spec(FilesystemKind::F2fs, None)),
            Err(StorageError::Refused(Refusal::MissingTool { tool, .. })) if tool == "f2fs-tools"
        ));
    }
}
