use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct ByteSize(pub u64);

const UNITS: [&str; 6] = ["bytes", "KB", "MB", "GB", "TB", "PB"];
const STEP: f64 = 1000.0;

impl ByteSize {
    pub const MIB: Self = Self(1 << 20);
    pub const GIB: Self = Self(1 << 30);

    pub const fn bytes(self) -> u64 {
        self.0
    }

    pub const fn align_down(self, alignment: Self) -> Self {
        Self(self.0 - self.0 % alignment.0)
    }

    pub const fn saturating_sub(self, other: Self) -> Self {
        Self(self.0.saturating_sub(other.0))
    }
}

impl fmt::Display for ByteSize {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut value = self.0 as f64;
        let mut unit = 0;
        while value >= STEP && unit < UNITS.len() - 1 {
            value /= STEP;
            unit += 1;
        }
        match unit {
            0 => write!(formatter, "{} {}", self.0, UNITS[0]),
            _ if value >= 100.0 => write!(formatter, "{value:.0} {}", UNITS[unit]),
            _ => write!(formatter, "{value:.1} {}", UNITS[unit]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_read_like_a_drive_label() {
        assert_eq!(ByteSize(512).to_string(), "512 bytes");
        assert_eq!(ByteSize(1_500_000).to_string(), "1.5 MB");
        assert_eq!(ByteSize(512_110_190_592).to_string(), "512 GB");
    }
}
