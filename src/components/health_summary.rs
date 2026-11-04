use crownos_disks::model::Health;
use crownui::kit::{StatusOptions, status};
use crownui::prelude::{Styled, View, hstack};

use super::text::body;
use crate::util::health_text::{health_label, health_sentence, health_tone};

const SUMMARY_GAP: f32 = 12.0;

pub fn health_pill(health: &Health) -> Option<impl View + use<>> {
    let reported = matches!(health, Health::Reported(_));
    reported.then(|| {
        status(health_label(health))
            .tone(health_tone(health))
            .shrink(0.0)
    })
}

pub fn health_summary(health: &Health) -> impl View + use<> {
    hstack((health_pill(health), body(health_sentence(health))))
        .gap(SUMMARY_GAP)
        .items_center()
}
