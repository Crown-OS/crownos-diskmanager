use std::fs::File;
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::fs::FileTypeExt;

nix::ioctl_read!(block_device_size, 0x12, 114, u64);

/// Size in bytes of a block device, or the length of a regular file.
pub(crate) fn device_size(file: &File) -> io::Result<u64> {
    let metadata = file.metadata()?;
    if !metadata.file_type().is_block_device() {
        return Ok(metadata.len());
    }
    let mut size = 0;
    // SAFETY: BLKGETSIZE64 writes exactly one u64 through the pointer, which outlives the call.
    unsafe { block_device_size(file.as_raw_fd(), &raw mut size) }.map_err(io::Error::from)?;
    Ok(size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regular_files_report_their_length() -> io::Result<()> {
        let file = tempfile_with_len(12_345)?;
        assert_eq!(device_size(&file)?, 12_345);
        Ok(())
    }

    fn tempfile_with_len(len: u64) -> io::Result<File> {
        let path = std::env::temp_dir().join(format!("crownos-block-size-{}", std::process::id()));
        let file = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)?;
        std::fs::remove_file(&path)?;
        file.set_len(len)?;
        Ok(file)
    }
}
