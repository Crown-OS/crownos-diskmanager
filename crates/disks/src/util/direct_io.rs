use std::fs::File;
use std::io;

use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};

/// Switches the descriptor to `O_DIRECT` when the device supports it, bypassing the page cache.
/// Returns whether direct I/O is now active.
pub(crate) fn enable(file: &File) -> bool {
    fcntl_getfl(file)
        .and_then(|flags| fcntl_setfl(file, flags | OFlags::DIRECT))
        .is_ok()
}

/// Runs `operation`, and when `O_DIRECT` alignment rules reject it, retries once with buffered I/O.
pub(crate) fn with_fallback<T>(
    file: &File,
    mut operation: impl FnMut() -> io::Result<T>,
) -> io::Result<T> {
    match operation() {
        Err(error) if error.kind() == io::ErrorKind::InvalidInput && disable(file)? => operation(),
        result => result,
    }
}

pub(crate) fn disable(file: &File) -> io::Result<bool> {
    let flags = fcntl_getfl(file)?;
    if !flags.contains(OFlags::DIRECT) {
        return Ok(false);
    }
    fcntl_setfl(file, flags - OFlags::DIRECT)?;
    Ok(true)
}
