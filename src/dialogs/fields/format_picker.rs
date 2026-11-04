use crownos_disks::model::{Capabilities, FilesystemKind};
use crownui::kit::{ButtonOptions, ButtonVariant, Choice, button, choice_list, icons};
use crownui::prelude::{Cx, Listeners, Runtime, Semantic, Signal, Styled, View, show, vstack};

const EVERYDAY_FORMATS: usize = 4;
const GAP: f32 = 6.0;

fn description(filesystem: &FilesystemKind) -> &'static str {
    match filesystem {
        FilesystemKind::Ext4 => "Best for CrownOS and other Linux systems.",
        FilesystemKind::Exfat => "Works with Windows, macOS, Linux, cameras and TVs.",
        FilesystemKind::Fat32 => "Works almost everywhere. Files can be at most 4 GB.",
        FilesystemKind::Ntfs => "Best for drives shared with Windows.",
        FilesystemKind::Btrfs => "Linux only, with snapshots and compression.",
        FilesystemKind::Xfs => "Linux only, tuned for very large files. Can't shrink.",
        FilesystemKind::F2fs => "Linux only, tuned for flash storage.",
        FilesystemKind::Swap => "Extra memory space for Linux. Holds no files.",
        _ => "",
    }
}

/// Why `filesystem` can't be created here, or `None` when it can. An empty capability list
/// means the service hasn't reported any, so nothing is ruled out.
pub fn unavailable_reason(
    capabilities: &Capabilities,
    filesystem: &FilesystemKind,
) -> Option<String> {
    if capabilities.formats.is_empty() {
        return None;
    }
    match capabilities
        .formats
        .iter()
        .find(|support| &support.filesystem == filesystem)
    {
        Some(support) => support
            .missing_package
            .as_ref()
            .map(|tool| format!("Requires the {tool} package.")),
        None => Some("Not supported on this system.".to_owned()),
    }
}

fn choice(capabilities: &Capabilities, filesystem: &FilesystemKind) -> Choice {
    match unavailable_reason(capabilities, filesystem) {
        Some(reason) => Choice::new(filesystem.display_name(), reason).disabled(),
        None => Choice::new(filesystem.display_name(), description(filesystem)),
    }
}

/// The formats offered by Erase, New Partition and Format: the everyday four, and the rest
/// behind "More formats".
#[derive(Clone, Copy)]
pub struct FormatPicker {
    pub selected: Signal<Option<usize>>,
    more: Signal<bool>,
}

impl FormatPicker {
    pub fn new(cx: &mut Cx, preferred: &FilesystemKind, capabilities: &Capabilities) -> Self {
        let available =
            |filesystem: &FilesystemKind| unavailable_reason(capabilities, filesystem).is_none();
        let index = FilesystemKind::FORMATTABLE
            .iter()
            .position(|filesystem| filesystem == preferred && available(filesystem))
            .or_else(|| FilesystemKind::FORMATTABLE.iter().position(available));
        Self {
            selected: cx.signal(index),
            more: cx.signal(index.is_some_and(|index| index >= EVERYDAY_FORMATS)),
        }
    }

    pub fn filesystem(self, runtime: &mut Runtime) -> Option<FilesystemKind> {
        self.selected
            .get(runtime)
            .and_then(|index| FilesystemKind::FORMATTABLE.get(index).cloned())
    }

    pub fn view(self, capabilities: &Capabilities) -> impl View + use<> {
        let choices: Vec<Choice> = FilesystemKind::FORMATTABLE
            .iter()
            .map(|filesystem| choice(capabilities, filesystem))
            .collect();
        let everyday: Vec<Choice> = choices.iter().take(EVERYDAY_FORMATS).cloned().collect();
        let Self { selected, more } = self;
        show(
            more,
            move || choice_list(choices.clone(), selected).label("Format"),
            move || {
                vstack((
                    choice_list(everyday.clone(), selected).label("Format"),
                    button("More formats")
                        .variant(ButtonVariant::Ghost)
                        .leading_icon(icons::CHEVRON_DOWN)
                        .on_click(move |cx, _| more.set(cx, true)),
                ))
                .gap(GAP)
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use crownos_disks::model::FormatSupport;

    use super::*;

    fn capabilities() -> Capabilities {
        Capabilities {
            formats: vec![
                FormatSupport {
                    filesystem: FilesystemKind::Ext4,
                    missing_package: None,
                },
                FormatSupport {
                    filesystem: FilesystemKind::Exfat,
                    missing_package: Some("exfatprogs".into()),
                },
            ],
            ..Capabilities::default()
        }
    }

    #[test]
    fn a_missing_tool_names_its_package() {
        let reason = unavailable_reason(&capabilities(), &FilesystemKind::Exfat);
        assert_eq!(reason.as_deref(), Some("Requires the exfatprogs package."));
        assert_eq!(
            unavailable_reason(&capabilities(), &FilesystemKind::Ext4),
            None
        );
    }

    #[test]
    fn unlisted_formats_are_unsupported_unless_nothing_was_reported() {
        assert!(unavailable_reason(&capabilities(), &FilesystemKind::Ntfs).is_some());
        assert_eq!(
            unavailable_reason(&Capabilities::default(), &FilesystemKind::Ntfs),
            None
        );
    }

    #[test]
    fn every_offered_format_is_described() {
        assert!(
            FilesystemKind::FORMATTABLE
                .iter()
                .all(|filesystem| !description(filesystem).is_empty())
        );
    }
}
