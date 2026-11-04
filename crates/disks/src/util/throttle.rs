use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

/// Lets an action run at most once per `interval` for each key.
pub(crate) struct Throttle<Key> {
    interval: Duration,
    last_run: Mutex<HashMap<Key, Instant>>,
}

impl<Key: Eq + Hash + Clone> Throttle<Key> {
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            last_run: Mutex::default(),
        }
    }

    /// Records a run and returns `true` unless `key` ran within the interval.
    pub fn try_begin(&self, key: &Key) -> bool {
        let now = Instant::now();
        let mut last_run = self.last_run.lock().unwrap_or_else(PoisonError::into_inner);
        let due = last_run
            .get(key)
            .is_none_or(|previous| now.duration_since(*previous) >= self.interval);
        if due {
            last_run.insert(key.clone(), now);
        }
        due
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_once_per_interval_per_key() {
        let throttle = Throttle::new(Duration::from_secs(300));
        assert!(throttle.try_begin(&"nvme0"));
        assert!(!throttle.try_begin(&"nvme0"));
        assert!(throttle.try_begin(&"sda"));
        assert!(Throttle::new(Duration::ZERO).try_begin(&"nvme0"));
    }
}
