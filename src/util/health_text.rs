use crownos_disks::model::{Health, HealthNote, HealthStatus};
use crownui::kit::Tone;

pub fn health_tone(health: &Health) -> Tone {
    match health {
        Health::Unavailable => Tone::Neutral,
        Health::Reported(report) => match report.status {
            HealthStatus::Good => Tone::Success,
            HealthStatus::Caution => Tone::Warning,
            HealthStatus::Failing => Tone::Danger,
        },
    }
}

pub fn health_label(health: &Health) -> &'static str {
    match health {
        Health::Unavailable => "Health unknown",
        Health::Reported(report) => match report.status {
            HealthStatus::Good => "Good",
            HealthStatus::Caution => "Caution",
            HealthStatus::Failing => "Failing",
        },
    }
}

pub fn health_sentence(health: &Health) -> &'static str {
    match health {
        Health::Unavailable => "Health isn't available over this connection.",
        Health::Reported(report) => match report.status {
            HealthStatus::Good => "This disk is working normally.",
            HealthStatus::Caution => {
                "This disk reports a problem. Keep a backup of important files."
            }
            HealthStatus::Failing => "This disk is failing. Back up your files now and replace it.",
        },
    }
}

pub fn note_text(note: &HealthNote) -> &'static str {
    match note {
        HealthNote::SpareBelowThreshold => "Spare capacity is used up.",
        HealthNote::TemperatureHigh => "The disk is running hot.",
        HealthNote::EnduranceReached => "The disk has reached its rated endurance.",
        HealthNote::MediaErrors => "The disk has reported errors reading or writing data.",
        HealthNote::ReadOnly => "The disk has switched itself to read-only.",
        HealthNote::Degraded => "The disk's reliability is degraded.",
        HealthNote::BadSectors => "The disk has bad sectors.",
        HealthNote::AttributesFailing => "Some health checks are failing.",
        HealthNote::SmartFailing => "The disk predicts that it will fail soon.",
    }
}

#[cfg(test)]
mod tests {
    use crownos_disks::model::HealthReport;

    use super::*;

    fn reported(status: HealthStatus) -> Health {
        Health::Reported(HealthReport {
            status,
            temperature_celsius: None,
            power_on_hours: None,
            life_used_percent: None,
            spare_percent: None,
            media_errors: None,
            unsafe_shutdowns: None,
            bad_sectors: None,
            notes: Vec::new(),
        })
    }

    #[test]
    fn status_maps_to_tone() {
        assert_eq!(health_tone(&reported(HealthStatus::Good)), Tone::Success);
        assert_eq!(health_tone(&reported(HealthStatus::Caution)), Tone::Warning);
        assert_eq!(health_tone(&reported(HealthStatus::Failing)), Tone::Danger);
        assert_eq!(health_tone(&Health::Unavailable), Tone::Neutral);
    }
}
