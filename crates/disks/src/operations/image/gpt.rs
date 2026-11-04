use std::fs::File;
use std::io;

use gptman::GPT;

use crate::util::direct_io;

/// Moves the backup GPT header of an image restored to a larger disk to the disk's last sector.
pub(super) fn relocate_backup_header(device: &File) -> io::Result<()> {
    direct_io::disable(device)?;
    let mut device_io = device;
    let mut table = match GPT::find_from(&mut device_io) {
        Ok(table) => table,
        Err(gptman::Error::InvalidSignature) => return Ok(()),
        Err(error) => return Err(io::Error::other(error)),
    };
    let last_lba = crate::util::block_size::device_size(device)? / table.sector_size;
    let last_lba = last_lba.saturating_sub(1);
    if table.header.backup_lba == last_lba {
        return Ok(());
    }
    table
        .header
        .update_from(&mut device_io, table.sector_size)
        .map_err(io::Error::other)?;
    table.write_into(&mut device_io).map_err(io::Error::other)?;
    device.sync_all()
}
