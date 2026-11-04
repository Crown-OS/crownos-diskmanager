use std::fs::File;
use std::io;
use std::os::unix::fs::FileExt;

const FOOTER_LEN: u64 = 12;
const FOOTER_MAGIC: &[u8; 2] = b"YZ";
const INDEX_INDICATOR: u8 = 0;
const MAX_INDEX_LEN: u64 = 16 << 20;
const MAX_VLI_BYTES: usize = 9;

/// Uncompressed size recorded in the index of the file's last xz stream, when it can be read.
pub(super) fn uncompressed_size(file: &File, file_len: u64) -> io::Result<Option<u64>> {
    let Some(footer_offset) = file_len.checked_sub(FOOTER_LEN) else {
        return Ok(None);
    };
    let mut footer = [0; FOOTER_LEN as usize];
    file.read_exact_at(&mut footer, footer_offset)?;
    if &footer[10..] != FOOTER_MAGIC {
        return Ok(None);
    }
    let stored_backward_size = u32::from_le_bytes([footer[4], footer[5], footer[6], footer[7]]);
    let index_len = (u64::from(stored_backward_size) + 1) * 4;
    let Some(index_offset) = footer_offset
        .checked_sub(index_len)
        .filter(|_| index_len <= MAX_INDEX_LEN)
    else {
        return Ok(None);
    };
    let mut index = vec![0; index_len as usize];
    file.read_exact_at(&mut index, index_offset)?;
    Ok(sum_records(&index))
}

fn sum_records(index: &[u8]) -> Option<u64> {
    let (&indicator, mut records) = index.split_first()?;
    if indicator != INDEX_INDICATOR {
        return None;
    }
    let count = read_vli(&mut records)?;
    (0..count).try_fold(0u64, |total, _| {
        read_vli(&mut records)?;
        total.checked_add(read_vli(&mut records)?)
    })
}

fn read_vli(bytes: &mut &[u8]) -> Option<u64> {
    let mut value = 0;
    for (position, &byte) in bytes.iter().take(MAX_VLI_BYTES).enumerate() {
        value |= u64::from(byte & 0x7F) << (7 * position);
        if byte & 0x80 == 0 {
            *bytes = &bytes[position + 1..];
            return Some(value);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_multi_byte_integers() {
        let mut bytes: &[u8] = &[0xE5, 0x8E, 0x26, 0x01];
        assert_eq!(read_vli(&mut bytes), Some(624_485));
        assert_eq!(bytes, &[0x01]);
    }

    #[test]
    fn sums_every_record() {
        let index = [INDEX_INDICATOR, 2, 10, 0x80, 0x01, 5, 0x20];
        assert_eq!(sum_records(&index), Some(128 + 0x20));
    }
}
