use std::collections::HashMap;
use std::convert::Infallible;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use zbus::{OwnedVariant, Variant};

use super::udisks::{JOB, call, options};
use crate::client::Session;
use crate::command::Progress;
use crate::error::StorageError;
use crate::operations::OperationContext;

const POLL_INTERVAL: Duration = Duration::from_millis(500);
const MICROSECONDS_PER_SECOND: u64 = 1_000_000;

/// A UDisks2 job acting on one of the followed objects.
#[derive(Debug, Clone, PartialEq)]
struct JobStatus {
    path: String,
    progress: Progress,
    cancelable: bool,
}

/// Runs `work` while reporting the progress of the UDisks2 job it starts on any of `objects`,
/// and cancels that job when the operation is cancelled.
pub(crate) async fn follow<T>(
    session: &Session,
    context: &OperationContext,
    phase: &str,
    objects: &[&str],
    work: impl Future<Output = Result<T, StorageError>>,
) -> Result<T, StorageError> {
    context.ensure_not_cancelled()?;
    context.report(phase, Progress::default());
    tokio::select! {
        result = work => result,
        never = watch(session, context, phase, objects) => match never {},
    }
}

async fn watch(
    session: &Session,
    context: &OperationContext,
    phase: &str,
    objects: &[&str],
) -> Infallible {
    let mut cancel_requested = false;
    loop {
        tokio::time::sleep(POLL_INTERVAL).await;
        let Some(job) = find_job(session, objects).await else {
            continue;
        };
        context.report(phase, job.progress);
        if context.is_cancelled() && job.cancelable && !cancel_requested {
            cancel_requested = call::<_, ()>(session, &job.path, JOB, "Cancel", (options(),))
                .await
                .is_ok();
        }
    }
}

async fn find_job(session: &Session, objects: &[&str]) -> Option<JobStatus> {
    let managed = session
        .client
        .object_manager()
        .get_managed_objects()
        .await
        .ok()?;
    managed.iter().find_map(|(path, interfaces)| {
        let properties = interfaces.iter().find(|(name, _)| name.as_str() == JOB)?.1;
        job_status(path.as_str(), properties, objects)
    })
}

fn job_status(
    path: &str,
    properties: &HashMap<String, OwnedVariant>,
    objects: &[&str],
) -> Option<JobStatus> {
    let property = |name: &str| properties.get(name).map(|value| &**value);
    let targets = match property("Objects")? {
        Variant::Array(array) => array.iter().any(|item| matches!(item, Variant::ObjectPath(target) if objects.contains(&target.as_str()))),
        _ => false,
    };
    if !targets {
        return None;
    }
    let flag = |name: &str| matches!(property(name), Some(Variant::Bool(true)));
    let unsigned = |name: &str| match property(name) {
        Some(Variant::U64(value)) => Some(*value).filter(|value| *value > 0),
        _ => None,
    };
    let fraction = match property("Progress") {
        Some(Variant::F64(fraction)) if flag("ProgressValid") => {
            Some(fraction.clamp(0.0, 1.0) as f32)
        }
        _ => None,
    };
    Some(JobStatus {
        path: path.to_owned(),
        progress: Progress {
            fraction,
            bytes_per_second: unsigned("Rate"),
            seconds_remaining: unsigned("ExpectedEndTime").and_then(seconds_until),
        },
        cancelable: flag("Cancelable"),
    })
}

fn seconds_until(end_microseconds: u64) -> Option<u64> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_micros();
    let remaining = u128::from(end_microseconds).checked_sub(now)?;
    u64::try_from(remaining / u128::from(MICROSECONDS_PER_SECOND)).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use zbus::{Array, ObjectPath};

    const TARGET: &str = "/org/freedesktop/UDisks2/block_devices/sdb1";

    fn properties(objects: &[&str], progress: f64, valid: bool) -> HashMap<String, OwnedVariant> {
        let paths: Vec<ObjectPath<'_>> = objects
            .iter()
            .filter_map(|path| ObjectPath::try_from(*path).ok())
            .collect();
        let owned = |value: Variant<'_>| OwnedVariant::try_from(value).expect("owned value");
        HashMap::from([
            (
                "Objects".to_owned(),
                owned(Variant::Array(Array::from(paths))),
            ),
            ("Progress".to_owned(), owned(Variant::F64(progress))),
            ("ProgressValid".to_owned(), owned(Variant::Bool(valid))),
            ("Rate".to_owned(), owned(Variant::U64(4_000_000))),
            ("ExpectedEndTime".to_owned(), owned(Variant::U64(0))),
            ("Cancelable".to_owned(), owned(Variant::Bool(true))),
        ])
    }

    #[test]
    fn reads_progress_of_jobs_on_followed_objects() {
        let status =
            job_status("/jobs/1", &properties(&[TARGET], 0.25, true), &[TARGET]).expect("job");
        assert_eq!(status.progress.fraction, Some(0.25));
        assert_eq!(status.progress.bytes_per_second, Some(4_000_000));
        assert_eq!(status.progress.seconds_remaining, None);
        assert!(status.cancelable);
    }

    #[test]
    fn ignores_invalid_progress_and_other_objects() {
        let status =
            job_status("/jobs/1", &properties(&[TARGET], 0.25, false), &[TARGET]).expect("job");
        assert_eq!(status.progress.fraction, None);
        assert!(job_status("/jobs/1", &properties(&["/other"], 0.5, true), &[TARGET]).is_none());
    }

    #[test]
    fn remaining_time_counts_down_to_the_end() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_micros() as u64;
        assert!(matches!(
            seconds_until(now + 90 * MICROSECONDS_PER_SECOND),
            Some(88..=90)
        ));
        assert_eq!(
            seconds_until(now.saturating_sub(MICROSECONDS_PER_SECOND)),
            None
        );
    }
}
