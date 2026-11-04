use super::assessment::Assessment;
use super::{SECONDS_PER_HOUR, kelvin_to_celsius};
use crate::model::{Health, HealthNote, HealthReport, HealthStatus};
use crate::object_map::{Properties, Value};

const CAUTION_TEMPERATURE_CELSIUS: f32 = 60.0;

/// `Drive.Ata` SMART properties; UDisks reports unknown counts as -1.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct AtaSmart {
    pub supported: bool,
    pub enabled: bool,
    pub updated: u64,
    pub failing: bool,
    pub temperature_kelvin: f64,
    pub power_on_seconds: u64,
    pub bad_sectors: i64,
    pub attributes_failing: i64,
    pub attributes_failed_in_the_past: i64,
}

impl AtaSmart {
    pub fn from_properties(properties: &Properties) -> Self {
        let flag = |name| {
            properties
                .get(name)
                .and_then(Value::as_bool)
                .unwrap_or_default()
        };
        let count = |name| properties.get(name).and_then(Value::as_i64).unwrap_or(-1);
        Self {
            supported: flag("SmartSupported"),
            enabled: flag("SmartEnabled"),
            updated: properties
                .get("SmartUpdated")
                .and_then(Value::as_u64)
                .unwrap_or_default(),
            failing: flag("SmartFailing"),
            temperature_kelvin: properties
                .get("SmartTemperature")
                .and_then(Value::as_f64)
                .unwrap_or_default(),
            power_on_seconds: properties
                .get("SmartPowerOnSeconds")
                .and_then(Value::as_u64)
                .unwrap_or_default(),
            bad_sectors: count("SmartNumBadSectors"),
            attributes_failing: count("SmartNumAttributesFailing"),
            attributes_failed_in_the_past: count("SmartNumAttributesFailedInThePast"),
        }
    }

    pub fn report(&self) -> Health {
        if !self.supported || !self.enabled || self.updated == 0 {
            return Health::Unavailable;
        }
        let temperature_celsius =
            (self.temperature_kelvin > 0.0).then(|| kelvin_to_celsius(self.temperature_kelvin));
        let bad_sectors = u64::try_from(self.bad_sectors).ok();

        let mut assessment = Assessment::new();
        assessment.raise_if(
            self.failing,
            HealthStatus::Failing,
            HealthNote::SmartFailing,
        );
        let too_hot =
            temperature_celsius.is_some_and(|celsius| celsius >= CAUTION_TEMPERATURE_CELSIUS);
        assessment.raise_if(too_hot, HealthStatus::Caution, HealthNote::TemperatureHigh);
        assessment.raise_if(
            bad_sectors.is_some_and(|count| count > 0),
            HealthStatus::Caution,
            HealthNote::BadSectors,
        );
        let attributes_failing =
            self.attributes_failing > 0 || self.attributes_failed_in_the_past > 0;
        assessment.raise_if(
            attributes_failing,
            HealthStatus::Caution,
            HealthNote::AttributesFailing,
        );

        Health::Reported(HealthReport {
            status: assessment.status,
            temperature_celsius,
            power_on_hours: (self.power_on_seconds > 0)
                .then_some(self.power_on_seconds / SECONDS_PER_HOUR),
            life_used_percent: None,
            spare_percent: None,
            media_errors: None,
            unsafe_shutdowns: None,
            bad_sectors,
            notes: assessment.notes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEALTHY: AtaSmart = AtaSmart {
        supported: true,
        enabled: true,
        updated: 1,
        failing: false,
        temperature_kelvin: 308.15,
        power_on_seconds: 36_000,
        bad_sectors: 0,
        attributes_failing: 0,
        attributes_failed_in_the_past: 0,
    };

    #[test]
    fn status_table() {
        let cases = [
            ("healthy", HEALTHY, HealthStatus::Good, vec![]),
            (
                "failing",
                AtaSmart {
                    failing: true,
                    ..HEALTHY
                },
                HealthStatus::Failing,
                vec![HealthNote::SmartFailing],
            ),
            (
                "hot",
                AtaSmart {
                    temperature_kelvin: 333.15,
                    ..HEALTHY
                },
                HealthStatus::Caution,
                vec![HealthNote::TemperatureHigh],
            ),
            (
                "bad sectors",
                AtaSmart {
                    bad_sectors: 8,
                    ..HEALTHY
                },
                HealthStatus::Caution,
                vec![HealthNote::BadSectors],
            ),
            (
                "failing attribute",
                AtaSmart {
                    attributes_failing: 1,
                    ..HEALTHY
                },
                HealthStatus::Caution,
                vec![HealthNote::AttributesFailing],
            ),
            (
                "failed in the past",
                AtaSmart {
                    attributes_failed_in_the_past: 2,
                    ..HEALTHY
                },
                HealthStatus::Caution,
                vec![HealthNote::AttributesFailing],
            ),
            (
                "unknown counts",
                AtaSmart {
                    bad_sectors: -1,
                    attributes_failing: -1,
                    ..HEALTHY
                },
                HealthStatus::Good,
                vec![],
            ),
            (
                "failing wins over caution",
                AtaSmart {
                    failing: true,
                    bad_sectors: 3,
                    ..HEALTHY
                },
                HealthStatus::Failing,
                vec![HealthNote::SmartFailing, HealthNote::BadSectors],
            ),
        ];
        for (name, smart, status, notes) in cases {
            let Health::Reported(report) = smart.report() else {
                panic!("{name}: expected a report");
            };
            assert_eq!((report.status, report.notes), (status, notes), "{name}");
        }
    }

    #[test]
    fn values_are_converted_once() {
        let Health::Reported(report) = HEALTHY.report() else {
            panic!("expected a report");
        };
        assert!((report.temperature_celsius.unwrap() - 35.0).abs() < 0.01);
        assert_eq!(report.power_on_hours, Some(10));
        assert_eq!(report.bad_sectors, Some(0));
    }

    #[test]
    fn disabled_or_unsupported_smart_is_unavailable() {
        assert_eq!(
            AtaSmart {
                enabled: false,
                ..HEALTHY
            }
            .report(),
            Health::Unavailable
        );
        assert_eq!(
            AtaSmart {
                supported: false,
                ..HEALTHY
            }
            .report(),
            Health::Unavailable
        );
        assert_eq!(
            AtaSmart {
                updated: 0,
                ..HEALTHY
            }
            .report(),
            Health::Unavailable
        );
    }
}
