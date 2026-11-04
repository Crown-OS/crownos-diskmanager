use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{ByteSize, FilesystemKind};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Volume {
    pub filesystem: FilesystemKind,
    pub label: String,
    pub uuid: String,
    pub mount_points: Vec<PathBuf>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    pub total: ByteSize,
    pub used: ByteSize,
}

impl Usage {
    pub const fn available(self) -> ByteSize {
        self.total.saturating_sub(self.used)
    }

    pub fn fraction_used(self) -> f32 {
        match self.total.0 {
            0 => 0.0,
            total => (self.used.0 as f64 / total as f64) as f32,
        }
    }
}

impl Volume {
    pub fn is_mounted(&self) -> bool {
        !self.mount_points.is_empty()
    }
}
