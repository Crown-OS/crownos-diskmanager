use super::assessment::Assessment;
use super::kelvin_to_celsius;
use crate::model::{Health, HealthNote, HealthReport, HealthStatus};
use crate::object_map::{Properties, Value};

const ENDURANCE_LIMIT_PERCENT: u8 = 100;

/// `NVMe.Controller` SMART properties.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct NvmeSmart<'map> {
    pub updated: u64,
    pub critical_warnings: &'map [String],
    pub temperature_kelvin: u64,
    pub power_on_hours: u64,
    pub attributes: Option<NvmeAttributes>,
}

/// The `SmartGetAttributes` values health derivation uses.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct NvmeAttributes {
    pub percent_used: Option<u8>,
    pub available_spare: Option<u8>,
    pub spare_threshold: Option<u8>,
    pub media_errors: Option<u64>,
    pub unsafe_shutdowns: Option<u64>,
    pub warning_temperature_kelvin: Option<u64>,
}

fn uint(properties: &Properties, name: &str) -> Option<u64> {
    properties.get(name).and_then(Value::as_u64)
}

fn percent(properties: &Properties, name: &str) -> Option<u8> {
    uint(properties, name).and_then(|value| u8::try_from(value).ok())
}

impl NvmeAttributes {
    pub fn from_properties(properties: &Properties) -> Self {
        Self {
            percent_used: percent(properties, "percent_used"),
            available_spare: percent(properties, "avail_spare"),
            spare_threshold: percent(properties, "spare_thresh"),
            media_errors: uint(properties, "media_errors"),
            unsafe_shutdowns: uint(properties, "unsafe_shutdowns"),
            warning_temperature_kelvin: uint(properties, "wctemp").filter(|kelvin| *kelvin > 0),
        }
    }
}

impl<'map> NvmeSmart<'map> {
    pub fn from_properties(
        controller: &'map Properties,
        attributes: Option<NvmeAttributes>,
    ) -> Self {
        Self {
            updated: uint(controller, "SmartUpdated").unwrap_or_default(),
            critical_warnings: controller
                .get("SmartCriticalWarning")
                .and_then(Value::as_list)
                .unwrap_or_default(),
            temperature_kelvin: uint(controller, "SmartTemperature").unwrap_or_default(),
            power_on_hours: uint(controller, "SmartPowerOnHours").unwrap_or_default(),
            attributes,
        }
    }

    pub fn report(&self) -> Health {
        if self.updated == 0 {
            return Health::Unavailable;
        }
        let attributes = self.attributes.unwrap_or_default();
        let mut assessment = Assessment::new();
        self.critical_warnings.iter().for_each(|warning| {
            assessment.raise_if(true, warning_status(warning), warning_note(warning))
        });

        let spare_below_threshold = attributes
            .available_spare
            .zip(attributes.spare_threshold)
            .is_some_and(|(spare, threshold)| spare < threshold);
        assessment.raise_if(
            spare_below_threshold,
            HealthStatus::Failing,
            HealthNote::SpareBelowThreshold,
        );
        let too_hot = attributes
            .warning_temperature_kelvin
            .is_some_and(|limit| self.temperature_kelvin >= limit);
        assessment.raise_if(too_hot, HealthStatus::Caution, HealthNote::TemperatureHigh);
        let worn_out = attributes
            .percent_used
            .is_some_and(|used| used >= ENDURANCE_LIMIT_PERCENT);
        assessment.raise_if(
            worn_out,
            HealthStatus::Caution,
            HealthNote::EnduranceReached,
        );
        let media_errors = attributes.media_errors.is_some_and(|errors| errors > 0);
        assessment.raise_if(media_errors, HealthStatus::Caution, HealthNote::MediaErrors);

        Health::Reported(HealthReport {
            status: assessment.status,
            temperature_celsius: (self.temperature_kelvin > 0)
                .then(|| kelvin_to_celsius(self.temperature_kelvin as f64)),
            power_on_hours: Some(self.power_on_hours),
            life_used_percent: attributes.percent_used,
            spare_percent: attributes.available_spare,
            media_errors: attributes.media_errors,
            unsafe_shutdowns: attributes.unsafe_shutdowns,
            bad_sectors: None,
            notes: assessment.notes,
        })
    }
}

fn warning_status(warning: &str) -> HealthStatus {
    match warning {
        "temperature" => HealthStatus::Caution,
        _ => HealthStatus::Failing,
    }
}

fn warning_note(warning: &str) -> HealthNote {
    match warning {
        "spare" => HealthNote::SpareBelowThreshold,
        "temperature" => HealthNote::TemperatureHigh,
        "readonly" | "pmr_readonly" => HealthNote::ReadOnly,
        _ => HealthNote::Degraded,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEALTHY: NvmeAttributes = NvmeAttributes {
        percent_used: Some(6),
        available_spare: Some(100),
        spare_threshold: Some(5),
        media_errors: Some(0),
        unsafe_shutdowns: Some(350),
        warning_temperature_kelvin: Some(356),
    };

    type Case = (
        &'static str,
        Vec<String>,
        u64,
        NvmeAttributes,
        HealthStatus,
        Vec<HealthNote>,
    );

    fn report(
        warnings: &[String],
        temperature_kelvin: u64,
        attributes: NvmeAttributes,
    ) -> HealthReport {
        let smart = NvmeSmart {
            updated: 1,
            critical_warnings: warnings,
            temperature_kelvin,
            power_on_hours: 5487,
            attributes: Some(attributes),
        };
        match smart.report() {
            Health::Reported(report) => report,
            Health::Unavailable => panic!("expected a report"),
        }
    }

    #[test]
    fn status_table() {
        let warnings = |names: &[&str]| {
            names
                .iter()
                .map(|name| (*name).to_owned())
                .collect::<Vec<_>>()
        };
        let cases: [Case; 9] = [
            ("healthy", vec![], 315, HEALTHY, HealthStatus::Good, vec![]),
            (
                "spare warning",
                warnings(&["spare"]),
                315,
                HEALTHY,
                HealthStatus::Failing,
                vec![HealthNote::SpareBelowThreshold],
            ),
            (
                "temperature warning",
                warnings(&["temperature"]),
                315,
                HEALTHY,
                HealthStatus::Caution,
                vec![HealthNote::TemperatureHigh],
            ),
            (
                "degraded",
                warnings(&["degraded"]),
                315,
                HEALTHY,
                HealthStatus::Failing,
                vec![HealthNote::Degraded],
            ),
            (
                "read only",
                warnings(&["readonly", "pmr_readonly"]),
                315,
                HEALTHY,
                HealthStatus::Failing,
                vec![HealthNote::ReadOnly],
            ),
            (
                "spare under threshold",
                vec![],
                315,
                NvmeAttributes {
                    available_spare: Some(3),
                    ..HEALTHY
                },
                HealthStatus::Failing,
                vec![HealthNote::SpareBelowThreshold],
            ),
            (
                "at warning temperature",
                vec![],
                356,
                HEALTHY,
                HealthStatus::Caution,
                vec![HealthNote::TemperatureHigh],
            ),
            (
                "endurance reached",
                vec![],
                315,
                NvmeAttributes {
                    percent_used: Some(100),
                    ..HEALTHY
                },
                HealthStatus::Caution,
                vec![HealthNote::EnduranceReached],
            ),
            (
                "media errors",
                vec![],
                315,
                NvmeAttributes {
                    media_errors: Some(2),
                    ..HEALTHY
                },
                HealthStatus::Caution,
                vec![HealthNote::MediaErrors],
            ),
        ];
        for (name, warnings, temperature, attributes, status, notes) in cases {
            let report = report(&warnings, temperature, attributes);
            assert_eq!((report.status, report.notes), (status, notes), "{name}");
        }
    }

    #[test]
    fn values_are_converted_once() {
        let report = report(&[], 315, HEALTHY);
        assert!((report.temperature_celsius.unwrap() - 41.85).abs() < 0.01);
        assert_eq!(report.power_on_hours, Some(5487));
        assert_eq!(report.life_used_percent, Some(6));
        assert_eq!(report.spare_percent, Some(100));
        assert_eq!(report.unsafe_shutdowns, Some(350));
    }

    #[test]
    fn never_read_is_unavailable() {
        assert_eq!(NvmeSmart::default().report(), Health::Unavailable);
    }
}
