use std::time::{Duration, Instant};

use crate::command::Progress;

const REPORT_INTERVAL: Duration = Duration::from_millis(200);

/// Turns a running byte count into throttled [`Progress`] samples.
pub(super) struct ProgressMeter {
    expected_bytes: Option<u64>,
    started: Instant,
    last_report: Option<Instant>,
}

impl ProgressMeter {
    pub(super) const fn new(expected_bytes: Option<u64>, started: Instant) -> Self {
        Self {
            expected_bytes,
            started,
            last_report: None,
        }
    }

    pub(super) fn sample(&mut self, done: u64, now: Instant) -> Option<Progress> {
        if self
            .last_report
            .is_some_and(|last| now.duration_since(last) < REPORT_INTERVAL)
        {
            return None;
        }
        self.last_report = Some(now);
        Some(self.progress(done, now))
    }

    fn progress(&self, done: u64, now: Instant) -> Progress {
        let elapsed = now.duration_since(self.started).as_secs_f64();
        let rate = (elapsed > 0.0 && done > 0).then(|| done as f64 / elapsed);
        let expected = self.expected_bytes.filter(|&total| total > 0);
        Progress {
            fraction: expected.map(|total| (done as f64 / total as f64).min(1.0) as f32),
            bytes_per_second: rate.map(|rate| rate as u64),
            seconds_remaining: expected
                .zip(rate)
                .map(|(total, rate)| (total.saturating_sub(done) as f64 / rate).ceil() as u64),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_at_most_every_interval() {
        let started = Instant::now();
        let mut meter = ProgressMeter::new(Some(1000), started);
        assert!(meter.sample(0, started).is_some());
        assert!(
            meter
                .sample(100, started + Duration::from_millis(150))
                .is_none()
        );
        let progress = meter.sample(500, started + Duration::from_secs(1));
        assert_eq!(
            progress,
            Some(Progress {
                fraction: Some(0.5),
                bytes_per_second: Some(500),
                seconds_remaining: Some(1)
            })
        );
        assert!(
            meter
                .sample(600, started + Duration::from_millis(1100))
                .is_none()
        );
        assert!(
            meter
                .sample(700, started + Duration::from_millis(1200))
                .is_some()
        );
    }

    #[test]
    fn unknown_totals_still_report_a_rate() {
        let started = Instant::now();
        let mut meter = ProgressMeter::new(None, started);
        let progress = meter.sample(2000, started + Duration::from_secs(2));
        assert_eq!(
            progress,
            Some(Progress {
                fraction: None,
                bytes_per_second: Some(1000),
                seconds_remaining: None
            })
        );
    }
}
