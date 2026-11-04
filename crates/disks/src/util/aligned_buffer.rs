use std::ops::{Deref, DerefMut};

pub(crate) const PAGE_SIZE: usize = 4096;

/// A zero-initialised byte buffer whose start is page aligned, as `O_DIRECT` I/O requires.
pub(crate) struct AlignedBuffer {
    storage: Box<[u8]>,
    start: usize,
    len: usize,
}

impl AlignedBuffer {
    pub(crate) fn new(len: usize) -> Self {
        let storage = vec![0; len + PAGE_SIZE].into_boxed_slice();
        let start = storage.as_ptr().align_offset(PAGE_SIZE).min(PAGE_SIZE);
        Self {
            storage,
            start,
            len,
        }
    }
}

impl Deref for AlignedBuffer {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.storage[self.start..self.start + self.len]
    }
}

impl DerefMut for AlignedBuffer {
    fn deref_mut(&mut self) -> &mut [u8] {
        &mut self.storage[self.start..self.start + self.len]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_on_a_page_boundary() {
        let buffer = AlignedBuffer::new(3 * PAGE_SIZE);
        assert_eq!(buffer.as_ptr().addr() % PAGE_SIZE, 0);
        assert_eq!(buffer.len(), 3 * PAGE_SIZE);
    }
}
