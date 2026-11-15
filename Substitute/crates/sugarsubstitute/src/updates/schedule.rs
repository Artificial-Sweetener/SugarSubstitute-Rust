// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decide when the running application checks for an update: at a first moment its launch
//! chooses, then periodically while it stays open, with growing backoff after a failed check, and
//! at once when the user asks to check now.
//!
//! The schedule is pure state over the execution owner's monotonic clock. It starts nothing and
//! owns no timer: the coordinator turns `due` into its native event-loop wake and admits the
//! check through its execution owner when that wake arrives.
use std::{error::Error, time::Duration};
use sugarsubstitute_contracts::execution::Moment;

/// Delay from application readiness to a standard build's first check, so startup work settles.
pub(crate) const FIRST_CHECK: Duration = Duration::from_secs(30);
/// Interval between checks after a check completes, while the application stays open.
pub(crate) const PERIOD: Duration = Duration::from_secs(4 * 60 * 60);
/// Delay before retrying the first failed check; each further failure doubles it.
pub(crate) const FIRST_RETRY: Duration = Duration::from_secs(60);
/// Longest delay between retries of a failing check.
pub(crate) const MAXIMUM_RETRY: Duration = Duration::from_secs(60 * 60);

/// Terminal result of one check attempt, as observed by its owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CheckOutcome {
    /// The check finished: the app is current, or an update was prepared or found to need
    /// another installer. The next check waits a full period.
    Completed,
    /// The check could not be admitted or ended with an error; it is retried with backoff.
    Failed,
}

/// What a request to check now did.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CheckRequest {
    /// The next check is due now.
    Due,
    /// A check is running; the request joins it instead of starting another.
    Joined,
}

/// One application lifetime's check timing. At most one check runs at a time.
#[derive(Debug)]
pub(crate) struct CheckSchedule {
    due: Option<Moment>,
    consecutive_failures: u32,
    running: bool,
}
impl CheckSchedule {
    /// Plan the first check `first` after `start`, the moment checking became possible.
    /// # Errors
    /// Rejects a moment that cannot represent the first due time.
    pub(crate) fn new(start: Moment, first: Duration) -> Result<Self, Box<dyn Error>> {
        Ok(Self { due: Some(start.checked_add(first)?), consecutive_failures: 0, running: false })
    }

    /// Moment at which the next check is due; `None` while a check is running.
    pub(crate) fn due(&self) -> Option<Moment> {
        self.due
    }

    /// Claim the due check at `now`. Returns `true` exactly once per due time; the caller must
    /// then report the attempt's outcome through `finish`, including a failed admission.
    pub(crate) fn begin(&mut self, now: Moment) -> bool {
        if self.running || self.due.is_none_or(|due| now < due) {
            return false;
        }
        self.running = true;
        self.due = None;
        true
    }

    /// Ask for a check at `now`. While a check runs the request joins it; otherwise the next check
    /// becomes due now, and its outcome plans the following one as usual.
    pub(crate) fn request(&mut self, now: Moment) -> CheckRequest {
        if self.running {
            return CheckRequest::Joined;
        }
        self.due = Some(self.due.map_or(now, |due| due.min(now)));
        CheckRequest::Due
    }

    /// Record a running check's outcome at `now` and plan the next one.
    /// # Errors
    /// Rejects an outcome without a running check, or an unrepresentable due time.
    pub(crate) fn finish(
        &mut self,
        outcome: CheckOutcome,
        now: Moment,
    ) -> Result<Moment, Box<dyn Error>> {
        if !self.running {
            return Err("No update check is running".into());
        }
        let delay = match outcome {
            CheckOutcome::Completed => {
                self.consecutive_failures = 0;
                PERIOD
            }
            CheckOutcome::Failed => {
                self.consecutive_failures = self.consecutive_failures.saturating_add(1);
                retry_delay(self.consecutive_failures)
            }
        };
        let due = now.checked_add(delay)?;
        self.running = false;
        self.due = Some(due);
        Ok(due)
    }
}

/// `FIRST_RETRY` doubled for each failure after the first, capped at `MAXIMUM_RETRY`.
fn retry_delay(failures: u32) -> Duration {
    let doublings = failures.saturating_sub(1).min(16);
    FIRST_RETRY.saturating_mul(1 << doublings).min(MAXIMUM_RETRY)
}

#[cfg(test)]
mod tests {
    use super::*;
    type Result = std::result::Result<(), Box<dyn Error>>;

    fn at(seconds: u64) -> Moment {
        Moment::from_nanos(seconds * 1_000_000_000)
    }

    #[test]
    fn first_check_is_due_shortly_after_readiness_and_runs_once() -> Result {
        let mut schedule = CheckSchedule::new(at(100), FIRST_CHECK)?;
        assert_eq!(schedule.due(), Some(at(130)));
        assert!(!schedule.begin(at(129)));
        assert!(schedule.begin(at(130)));
        assert_eq!(schedule.due(), None);
        assert!(!schedule.begin(at(10_000)));
        Ok(())
    }

    #[test]
    fn a_first_check_without_delay_is_due_at_once() -> Result {
        let mut schedule = CheckSchedule::new(at(5), Duration::ZERO)?;
        assert_eq!(schedule.due(), Some(at(5)));
        assert!(schedule.begin(at(5)));
        Ok(())
    }

    #[test]
    fn failed_checks_retry_on_a_doubling_backoff_capped_at_an_hour() -> Result {
        let mut schedule = CheckSchedule::new(at(0), FIRST_CHECK)?;
        let mut now = at(30);
        let mut delays = Vec::new();
        for _ in 0..9 {
            assert!(schedule.begin(now));
            let due = schedule.finish(CheckOutcome::Failed, now)?;
            delays.push(due.checked_duration_since(now)?.as_secs());
            assert!(!schedule.begin(at(due.as_nanos() / 1_000_000_000 - 1)));
            now = due;
        }
        assert_eq!(delays, [60, 120, 240, 480, 960, 1920, 3600, 3600, 3600]);
        Ok(())
    }

    #[test]
    fn a_completed_check_resets_backoff_and_the_next_check_follows_a_period_later() -> Result {
        let mut schedule = CheckSchedule::new(at(0), FIRST_CHECK)?;
        assert!(schedule.begin(at(30)));
        let retry = schedule.finish(CheckOutcome::Failed, at(40))?;
        assert_eq!(retry, at(100));
        assert!(schedule.begin(retry));
        let next = schedule.finish(CheckOutcome::Completed, at(150))?;
        assert_eq!(next, at(150 + 4 * 60 * 60));
        assert!(!schedule.begin(at(150 + 4 * 60 * 60 - 1)));
        assert!(schedule.begin(next));
        assert_eq!(schedule.finish(CheckOutcome::Failed, next)?, next.checked_add(FIRST_RETRY)?);
        Ok(())
    }

    #[test]
    fn a_request_makes_the_check_due_now_or_joins_the_running_one() -> Result {
        let mut schedule = CheckSchedule::new(at(0), FIRST_CHECK)?;
        assert_eq!(schedule.request(at(5)), CheckRequest::Due);
        assert_eq!(schedule.due(), Some(at(5)));
        assert!(schedule.begin(at(5)), "the requested check starts at once");
        assert_eq!(schedule.request(at(6)), CheckRequest::Joined);
        assert_eq!(schedule.due(), None, "a joined request plans no second check");
        assert!(!schedule.begin(at(6)));
        let next = schedule.finish(CheckOutcome::Failed, at(7))?;
        assert_eq!(next, at(7).checked_add(FIRST_RETRY)?);
        assert_eq!(schedule.request(at(8)), CheckRequest::Due);
        assert!(schedule.begin(at(8)), "a request does not wait out the retry backoff");
        Ok(())
    }

    #[test]
    fn an_outcome_without_a_running_check_is_rejected() -> Result {
        let mut schedule = CheckSchedule::new(at(0), FIRST_CHECK)?;
        assert!(schedule.finish(CheckOutcome::Completed, at(1)).is_err());
        assert_eq!(schedule.due(), Some(at(30)));
        Ok(())
    }
}
