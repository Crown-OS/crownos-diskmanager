use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Health {
    Unavailable,
    Reported(HealthReport),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealthReport {
    pub status: HealthStatus,
    pub temperature_celsius: Option<f32>,
    pub power_on_hours: Option<u64>,
    pub life_used_percent: Option<u8>,
    pub spare_percent: Option<u8>,
    pub media_errors: Option<u64>,
    pub unsafe_shutdowns: Option<u64>,
    pub bad_sectors: Option<u64>,
    pub notes: Vec<HealthNote>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum HealthStatus {
    Good,
    Caution,
    Failing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthNote {
    SpareBelowThreshold,
    TemperatureHigh,
    EnduranceReached,
    MediaErrors,
    ReadOnly,
    Degraded,
    BadSectors,
    AttributesFailing,
    SmartFailing,
}
