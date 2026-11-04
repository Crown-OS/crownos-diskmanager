use crownos_disks::command::Progress;
use crownos_disks::model::ByteSize;

const MINUTE: u64 = 60;
const HOUR: u64 = 60 * MINUTE;
const ALMOST_DONE: u64 = 5;

pub fn remaining_time(seconds: u64) -> String {
    match seconds {
        0..ALMOST_DONE => "Almost done".to_owned(),
        ALMOST_DONE..MINUTE => "Less than a minute left".to_owned(),
        MINUTE..HOUR => match seconds.div_ceil(MINUTE) {
            1 => "About 1 minute left".to_owned(),
            minutes => format!("About {minutes} minutes left"),
        },
        _ => {
            let (hours, minutes) = (seconds / HOUR, (seconds % HOUR).div_ceil(MINUTE));
            match minutes {
                0 => format!("About {hours} h left"),
                minutes => format!("About {hours} h {minutes} min left"),
            }
        }
    }
}

/// "45.2 MB/s · About 3 minutes left", or whichever half is known.
pub fn rate_and_remaining(progress: &Progress) -> String {
    let rate = progress
        .bytes_per_second
        .map(|rate| format!("{}/s", ByteSize(rate)));
    let remaining = progress.seconds_remaining.map(remaining_time);
    match (rate, remaining) {
        (Some(rate), Some(remaining)) => format!("{rate} · {remaining}"),
        (Some(text), None) | (None, Some(text)) => text,
        (None, None) => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remaining_time_reads_like_a_sentence() {
        assert_eq!(remaining_time(2), "Almost done");
        assert_eq!(remaining_time(40), "Less than a minute left");
        assert_eq!(remaining_time(61), "About 2 minutes left");
        assert_eq!(remaining_time(60), "About 1 minute left");
        assert_eq!(remaining_time(2 * HOUR), "About 2 h left");
        assert_eq!(remaining_time(HOUR + 90), "About 1 h 2 min left");
    }

    #[test]
    fn rate_and_remaining_join_what_is_known() {
        let both = Progress {
            fraction: Some(0.5),
            bytes_per_second: Some(45_200_000),
            seconds_remaining: Some(170),
        };
        assert_eq!(
            rate_and_remaining(&both),
            "45.2 MB/s · About 3 minutes left"
        );
        let rate = Progress {
            seconds_remaining: None,
            ..both
        };
        assert_eq!(rate_and_remaining(&rate), "45.2 MB/s");
        assert_eq!(rate_and_remaining(&Progress::default()), "");
    }
}
