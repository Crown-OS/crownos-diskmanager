use crownos_disks::model::ByteSize;

const DECIMAL_STEP: u64 = 1000;
const MAX_DECIMALS: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeUnit {
    Megabytes,
    Gigabytes,
    Terabytes,
}

impl SizeUnit {
    pub const ALL: [Self; 3] = [Self::Megabytes, Self::Gigabytes, Self::Terabytes];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Megabytes => "MB",
            Self::Gigabytes => "GB",
            Self::Terabytes => "TB",
        }
    }

    pub const fn bytes(self) -> u64 {
        match self {
            Self::Megabytes => DECIMAL_STEP.pow(2),
            Self::Gigabytes => DECIMAL_STEP.pow(3),
            Self::Terabytes => DECIMAL_STEP.pow(4),
        }
    }

    pub fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|unit| *unit == self)
            .unwrap_or_default()
    }

    pub fn from_index(index: usize) -> Self {
        Self::ALL.get(index).copied().unwrap_or(Self::Gigabytes)
    }

    pub fn fitting(size: ByteSize) -> Self {
        Self::ALL
            .into_iter()
            .rev()
            .find(|unit| size.bytes() >= unit.bytes())
            .unwrap_or(Self::Megabytes)
    }
}

fn suffix_multiplier(suffix: &str) -> Option<u64> {
    let multiplier = match suffix.to_ascii_lowercase().as_str() {
        "" => return None,
        "b" | "byte" | "bytes" => 1,
        "k" | "kb" => DECIMAL_STEP,
        "m" | "mb" => SizeUnit::Megabytes.bytes(),
        "g" | "gb" => SizeUnit::Gigabytes.bytes(),
        "t" | "tb" => SizeUnit::Terabytes.bytes(),
        "kib" => 1 << 10,
        "mib" => 1 << 20,
        "gib" => 1 << 30,
        "tib" => 1 << 40,
        _ => 0,
    };
    Some(multiplier)
}

/// Reads sizes such as "1.5 GB", "512 MiB" or "20" (in `unit`); `None` when it isn't a size.
pub fn parse_size(input: &str, unit: SizeUnit) -> Option<ByteSize> {
    let input = input.trim().replace(',', ".");
    let split = input
        .find(|character: char| !(character.is_ascii_digit() || character == '.'))
        .unwrap_or(input.len());
    let (number, suffix) = input.split_at(split);
    let value: f64 = number.parse().ok()?;
    let multiplier = suffix_multiplier(suffix.trim()).unwrap_or(unit.bytes());
    let bytes = value * multiplier as f64;
    (multiplier > 0 && bytes.is_finite() && bytes >= 0.0 && bytes < u64::MAX as f64)
        .then(|| ByteSize(bytes.round() as u64))
}

/// The size as a plain number in `unit`, with at most two decimals and no trailing zeros.
pub fn format_in_unit(size: ByteSize, unit: SizeUnit) -> String {
    let value = size.bytes() as f64 / unit.bytes() as f64;
    let text = format!("{value:.MAX_DECIMALS$}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// Rounds down to whole MiB and keeps the result within `min..=max`.
pub fn snap_size(size: ByteSize, min: ByteSize, max: ByteSize) -> ByteSize {
    let ceiling = max.align_down(ByteSize::MIB).max(min);
    size.align_down(ByteSize::MIB).clamp(min, ceiling)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GB: u64 = 1_000_000_000;

    #[test]
    fn numbers_take_the_selected_unit() {
        assert_eq!(
            parse_size("20", SizeUnit::Gigabytes),
            Some(ByteSize(20 * GB))
        );
        assert_eq!(
            parse_size(" 1.5 ", SizeUnit::Gigabytes),
            Some(ByteSize(1_500_000_000))
        );
        assert_eq!(
            parse_size("1,5", SizeUnit::Terabytes),
            Some(ByteSize(1_500 * GB))
        );
    }

    #[test]
    fn a_typed_suffix_wins_over_the_selected_unit() {
        assert_eq!(
            parse_size("1.5 GB", SizeUnit::Megabytes),
            Some(ByteSize(1_500_000_000))
        );
        assert_eq!(
            parse_size("512mib", SizeUnit::Gigabytes),
            Some(ByteSize(512 << 20))
        );
        assert_eq!(
            parse_size("2T", SizeUnit::Megabytes),
            Some(ByteSize(2_000 * GB))
        );
        assert_eq!(
            parse_size("300 MB", SizeUnit::Terabytes),
            Some(ByteSize(300_000_000))
        );
    }

    #[test]
    fn anything_else_is_not_a_size() {
        for input in ["", "GB", "abc", "1.2.3 GB", "5 parsecs", "-3"] {
            assert_eq!(parse_size(input, SizeUnit::Gigabytes), None, "{input:?}");
        }
    }

    #[test]
    fn formatting_drops_trailing_zeros() {
        assert_eq!(
            format_in_unit(ByteSize(1_500_000_000), SizeUnit::Gigabytes),
            "1.5"
        );
        assert_eq!(format_in_unit(ByteSize(64 * GB), SizeUnit::Gigabytes), "64");
        assert_eq!(
            format_in_unit(ByteSize(1_234_567_890), SizeUnit::Megabytes),
            "1234.57"
        );
    }

    #[test]
    fn the_fitting_unit_is_the_largest_not_above_the_size() {
        assert_eq!(SizeUnit::fitting(ByteSize(500_000)), SizeUnit::Megabytes);
        assert_eq!(SizeUnit::fitting(ByteSize(64 * GB)), SizeUnit::Gigabytes);
        assert_eq!(SizeUnit::fitting(ByteSize(2_000 * GB)), SizeUnit::Terabytes);
    }

    #[test]
    fn sizes_snap_to_whole_mebibytes_within_bounds() {
        let (min, max) = (ByteSize::MIB, ByteSize(64 * GB));
        assert_eq!(
            snap_size(ByteSize((5 << 20) + 123), min, max),
            ByteSize(5 << 20)
        );
        assert_eq!(snap_size(ByteSize(0), min, max), min);
        assert_eq!(
            snap_size(ByteSize(u64::MAX), min, max),
            max.align_down(ByteSize::MIB)
        );
    }

    #[test]
    fn units_round_trip_through_their_index() {
        for unit in SizeUnit::ALL {
            assert_eq!(SizeUnit::from_index(unit.index()), unit);
        }
    }
}
