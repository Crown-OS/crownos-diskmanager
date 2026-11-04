use crownos_disks::model::{Health, HealthReport};
use crownui::kit::{section, setting_row};
use crownui::prelude::{AnyView, Cx, Styled, View, vstack};

use crate::components::{health_summary, value_text};
use crate::util::health_text::note_text;
use crate::util::nothing::nothing;

const SUMMARY_PADDING_X: f32 = 18.0;
const SUMMARY_PADDING_Y: f32 = 14.0;

pub fn health(_cx: &mut Cx, health: Health) -> impl View + use<> {
    let summary = vstack(health_summary(&health))
        .px(SUMMARY_PADDING_X)
        .py(SUMMARY_PADDING_Y)
        .boxed();
    let rows = match &health {
        Health::Unavailable => Vec::new(),
        Health::Reported(report) => report_rows(report),
    };
    section("Health", (summary, rows))
}

fn report_rows(report: &HealthReport) -> Vec<AnyView> {
    let measurements = [
        (
            "Temperature",
            report
                .temperature_celsius
                .map(|celsius| format!("{celsius:.0} °C")),
        ),
        (
            "Life used",
            report
                .life_used_percent
                .map(|percent| format!("{percent} %")),
        ),
        (
            "Available spare",
            report.spare_percent.map(|percent| format!("{percent} %")),
        ),
        (
            "Powered on",
            report.power_on_hours.map(|hours| format!("{hours} hours")),
        ),
        (
            "Unsafe shutdowns",
            report.unsafe_shutdowns.map(|count| count.to_string()),
        ),
        (
            "Media errors",
            report.media_errors.map(|count| count.to_string()),
        ),
        (
            "Bad sectors",
            report.bad_sectors.map(|count| count.to_string()),
        ),
    ];
    measurements
        .into_iter()
        .filter_map(|(label, value)| {
            value.map(|value| setting_row(label, value_text(value)).boxed())
        })
        .chain(
            report
                .notes
                .iter()
                .map(|note| setting_row(note_text(note), nothing()).boxed()),
        )
        .collect()
}
