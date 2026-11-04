use std::fs::File;
use std::io::{self, Read};
use std::os::unix::fs::FileExt;
use std::path::Path;

use rustix::fs::{Advice, fadvise};

use super::{codec, xz_index};
use crate::error::StorageError;
use crate::model::ByteSize;

const XZ_MAGIC: &[u8] = &[0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00];
const ZSTD_MAGIC: &[u8] = &[0x28, 0xB5, 0x2F, 0xFD];
const ZSTD_MAX_FRAME_HEADER: usize = 18;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ImageFormat {
    Raw,
    Zstd,
    Xz,
}

impl ImageFormat {
    pub(super) fn detect(header: &[u8]) -> Self {
        match header {
            bytes if bytes.starts_with(XZ_MAGIC) => Self::Xz,
            bytes if bytes.starts_with(ZSTD_MAGIC) => Self::Zstd,
            _ => Self::Raw,
        }
    }
}

/// An image file opened for restoring, with its format and, when knowable, its decompressed size.
pub(super) struct ImageFile {
    file: File,
    pub format: ImageFormat,
    pub decompressed_size: Option<u64>,
}

impl ImageFile {
    pub(super) fn open(path: &Path) -> io::Result<Self> {
        let file = File::open(path)?;
        let file_len = file.metadata()?.len();
        let mut header = [0; ZSTD_MAX_FRAME_HEADER];
        let header_len = read_prefix(&file, &mut header)?;
        let header = &header[..header_len];
        let format = ImageFormat::detect(header);
        let decompressed_size = match format {
            ImageFormat::Raw => Some(file_len),
            ImageFormat::Zstd => zstd::zstd_safe::get_frame_content_size(header)
                .ok()
                .flatten(),
            ImageFormat::Xz => xz_index::uncompressed_size(&file, file_len)?,
        };
        Ok(Self {
            file,
            format,
            decompressed_size,
        })
    }

    pub(super) fn ensure_fits(&self, capacity: u64) -> Result<(), StorageError> {
        match self.decompressed_size {
            Some(size) if size > capacity => Err(StorageError::ImageTooLarge {
                capacity: ByteSize(capacity),
            }),
            _ => Ok(()),
        }
    }

    pub(super) fn into_reader(self) -> io::Result<Box<dyn Read + Send>> {
        fadvise(&self.file, 0, None, Advice::Sequential)?;
        Ok(match self.format {
            ImageFormat::Raw => Box::new(self.file),
            ImageFormat::Zstd => Box::new(codec::zstd_decoder(self.file)?),
            ImageFormat::Xz => Box::new(codec::xz_decoder(self.file)?),
        })
    }
}

fn read_prefix(file: &File, buffer: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buffer.len() {
        match file.read_at(&mut buffer[filled..], filled as u64)? {
            0 => break,
            read => filled += read,
        }
    }
    Ok(filled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_formats_by_magic() {
        assert_eq!(
            ImageFormat::detect(&[0xFD, b'7', b'z', b'X', b'Z', 0, 0]),
            ImageFormat::Xz
        );
        assert_eq!(
            ImageFormat::detect(&[0x28, 0xB5, 0x2F, 0xFD, 0x24]),
            ImageFormat::Zstd
        );
        assert_eq!(ImageFormat::detect(b"CD001 iso"), ImageFormat::Raw);
        assert_eq!(ImageFormat::detect(&[0x28, 0xB5]), ImageFormat::Raw);
    }
}
