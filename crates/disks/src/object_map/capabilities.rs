use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Results of the `Manager` capability queries, keyed by UDisks filesystem type.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagerCapabilities {
    #[serde(default)]
    pub supported_filesystems: Vec<String>,
    #[serde(default)]
    pub encryption_types: Vec<String>,
    /// `CanFormat(fs) -> (available, required_utility)`.
    #[serde(default)]
    pub can_format: BTreeMap<String, FormatProbe>,
    /// `CanResize(fs) -> (available, mode_flags, required_utility)`; absent when unsupported.
    #[serde(default)]
    pub can_resize: BTreeMap<String, ResizeProbe>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormatProbe {
    pub available: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub missing_tool: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResizeProbe {
    pub available: bool,
    pub modes: u64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub missing_tool: String,
}
