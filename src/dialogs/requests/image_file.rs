use std::path::{Path, PathBuf};

use crownos_disks::UserError;
use crownos_disks::command::Compression;
use crownos_disks::model::ByteSize;
use crownui::prelude::Cx;

use crate::dialogs::context::SheetContext;
use crate::util::portal::{FileChoice, choose_image};

const COMPRESSED_EXTENSIONS: [&str; 2] = ["xz", "zst"];
const DISC_FILESYSTEMS: [&str; 2] = ["iso9660", "udf"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageKind {
    Raw,
    Disc,
}

impl ImageKind {
    pub fn of_filesystem(udisks_type: &str) -> Self {
        match DISC_FILESYSTEMS.contains(&udisks_type) {
            true => Self::Disc,
            false => Self::Raw,
        }
    }
}

/// "SanDisk Ultra.img.zst", or "Ubuntu 24.04.iso" for disc content.
pub fn suggested_name(name: &str, kind: ImageKind, compression: Compression) -> String {
    let stem: String = name
        .chars()
        .map(|character| if character == '/' { '-' } else { character })
        .collect();
    let extension = match kind {
        ImageKind::Disc => "iso",
        ImageKind::Raw => "img",
    };
    let suffix = match compression {
        Compression::None => "",
        Compression::Zstd => ".zst",
        Compression::Xz => ".xz",
    };
    format!("{stem}.{extension}{suffix}")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChosenImage {
    pub path: PathBuf,
    pub size: Option<ByteSize>,
    pub compressed: bool,
}

impl ChosenImage {
    pub fn inspect(path: PathBuf) -> Self {
        let size = std::fs::metadata(&path)
            .ok()
            .map(|metadata| ByteSize(metadata.len()));
        let compressed = path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| COMPRESSED_EXTENSIONS.contains(&extension));
        Self {
            path,
            size,
            compressed,
        }
    }

    pub fn file_name(&self) -> String {
        display_name(&self.path)
    }

    /// Whether it can fit on `capacity`; compressed images are checked while writing.
    pub fn fits(&self, capacity: ByteSize) -> bool {
        self.compressed || self.size.is_none_or(|size| size <= capacity)
    }
}

pub fn display_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .into_owned()
}

fn portal_error(error: &anyhow::Error) -> UserError {
    UserError::new(
        "Couldn't open the file chooser",
        "The desktop's file chooser isn't available right now.",
    )
    .with_details(format!("{error:#}"))
}

/// Asks the desktop's file chooser for an image and hands the answer (`None` when cancelled)
/// to `on_done`; a missing portal shows the error sheet.
pub fn pick_image(
    cx: &mut Cx,
    context: SheetContext,
    title: &str,
    choice: FileChoice,
    on_done: impl FnOnce(&mut Cx, Option<PathBuf>) + 'static,
) {
    cx.spawn(
        choose_image(title.to_owned(), choice),
        move |cx, result| match result {
            Ok(path) => on_done(cx, path),
            Err(error) => {
                context.report(cx, portal_error(&error));
                on_done(cx, None);
            }
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggested_names_follow_content_and_compression() {
        assert_eq!(
            suggested_name("SanDisk Ultra", ImageKind::Raw, Compression::Zstd),
            "SanDisk Ultra.img.zst"
        );
        assert_eq!(
            suggested_name("Root", ImageKind::Raw, Compression::None),
            "Root.img"
        );
        assert_eq!(
            suggested_name("Backup", ImageKind::Raw, Compression::Xz),
            "Backup.img.xz"
        );
        assert_eq!(
            suggested_name("Ubuntu 24.04", ImageKind::Disc, Compression::None),
            "Ubuntu 24.04.iso"
        );
        assert_eq!(
            suggested_name("a/b", ImageKind::Raw, Compression::None),
            "a-b.img"
        );
    }

    #[test]
    fn disc_filesystems_make_disc_images() {
        assert_eq!(ImageKind::of_filesystem("iso9660"), ImageKind::Disc);
        assert_eq!(ImageKind::of_filesystem("udf"), ImageKind::Disc);
        assert_eq!(ImageKind::of_filesystem("ext4"), ImageKind::Raw);
    }

    #[test]
    fn only_raw_images_larger_than_the_target_are_refused() {
        let image = |size, compressed| ChosenImage {
            path: "/tmp/a.img".into(),
            size: Some(ByteSize(size)),
            compressed,
        };
        assert!(image(10, false).fits(ByteSize(10)));
        assert!(!image(11, false).fits(ByteSize(10)));
        assert!(image(11, true).fits(ByteSize(10)));
    }

    #[test]
    fn compression_is_read_from_the_extension() {
        assert!(ChosenImage::inspect("/nonexistent/a.img.zst".into()).compressed);
        assert!(ChosenImage::inspect("/nonexistent/a.img.xz".into()).compressed);
        assert!(!ChosenImage::inspect("/nonexistent/a.iso".into()).compressed);
        assert_eq!(ChosenImage::inspect("/nonexistent/a.iso".into()).size, None);
    }
}
