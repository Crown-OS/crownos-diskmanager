use crownos_disks::command::FormatSpec;
use crownos_disks::model::{Capabilities, FilesystemKind};
use crownui::prelude::{Cx, Runtime, View};

use super::{EncryptionFields, FORGOTTEN_PASSWORD_WARNING, FormatPicker, LabelField};
use crate::dialogs::sheets::layout::{form, labeled};

const NAME_PLACEHOLDER: &str = "Untitled";

/// Name, format and encryption: what Erase Disk, New Partition and Format all ask.
#[derive(Clone, Copy)]
pub struct VolumeForm {
    pub picker: FormatPicker,
    pub label: LabelField,
    pub encryption: EncryptionFields,
}

impl VolumeForm {
    pub fn new(
        cx: &mut Cx,
        preferred: &FilesystemKind,
        initial_label: String,
        capabilities: &Capabilities,
    ) -> Self {
        let picker = FormatPicker::new(cx, preferred, capabilities);
        let label = LabelField::new(cx, initial_label, move |runtime| {
            picker.filesystem(runtime).unwrap_or(FilesystemKind::Ext4)
        });
        let encryption_supported = capabilities.encryption;
        let encryption = EncryptionFields::new(cx, move |runtime| {
            encryption_supported
                && picker
                    .filesystem(runtime)
                    .is_some_and(|filesystem| filesystem.is_linux_native())
        });
        Self {
            picker,
            label,
            encryption,
        }
    }

    pub fn is_ready(self, runtime: &mut Runtime) -> bool {
        self.picker.filesystem(runtime).is_some()
            && self.label.is_valid(runtime)
            && self.encryption.is_ready(runtime)
    }

    pub fn warnings(self, runtime: &mut Runtime) -> Vec<&'static str> {
        match self.encryption.is_active(runtime) {
            true => vec![FORGOTTEN_PASSWORD_WARNING],
            false => Vec::new(),
        }
    }

    pub fn label_text(self, runtime: &mut Runtime) -> String {
        self.label.value.get(runtime)
    }

    pub fn filesystem_name(self, runtime: &mut Runtime) -> String {
        self.picker
            .filesystem(runtime)
            .map(|filesystem| filesystem.short_name().to_owned())
            .unwrap_or_default()
    }

    /// The format to apply, moving the password out of its fields.
    pub fn take_spec(self, runtime: &mut Runtime) -> Option<FormatSpec> {
        let filesystem = self.picker.filesystem(runtime)?;
        let label = self.label.value.get(runtime);
        let encryption = self.encryption.take_secret(runtime);
        Some(FormatSpec {
            filesystem,
            label,
            encryption,
        })
    }

    pub fn view(self, capabilities: &Capabilities) -> impl View + use<> {
        form((
            labeled("Name", self.label.view(NAME_PLACEHOLDER)),
            labeled("Format", self.picker.view(capabilities)),
            self.encryption.view(),
        ))
    }
}
