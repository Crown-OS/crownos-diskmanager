/// Rounds `value` up to the next multiple of `alignment`, saturating at `u64::MAX`.
pub(crate) const fn align_up(value: u64, alignment: u64) -> u64 {
    match value % alignment {
        0 => value,
        remainder => value.saturating_add(alignment - remainder),
    }
}

#[cfg(test)]
mod tests {
    use super::align_up;

    #[test]
    fn rounds_up_to_alignment() {
        assert_eq!(align_up(0, 8), 0);
        assert_eq!(align_up(1, 8), 8);
        assert_eq!(align_up(8, 8), 8);
        assert_eq!(align_up(9, 8), 16);
    }
}
