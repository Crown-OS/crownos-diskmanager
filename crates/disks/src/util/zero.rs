const SCAN_WIDTH: usize = 256;

/// Reports whether every byte is zero, folding fixed-width blocks so the inner loop vectorises.
pub(crate) fn is_all_zero(bytes: &[u8]) -> bool {
    bytes
        .chunks(SCAN_WIDTH)
        .all(|block| block.iter().fold(0, |acc, &byte| acc | byte) == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_a_single_set_byte() {
        let mut bytes = vec![0; 10_000];
        assert!(is_all_zero(&bytes));
        bytes[9_999] = 1;
        assert!(!is_all_zero(&bytes));
        assert!(is_all_zero(&[]));
    }
}
