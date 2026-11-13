// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Give native qualification campaigns the real clock their product owners need, and end a hung
//! campaign through the shared no-progress guard instead of a fixed campaign deadline.
//!
//! A campaign drives real native owners, so it samples real time for the product's own family
//! budgets and gives native owners and child processes the absolute bound they require. That bound
//! is [`CUSTODY`], a hang bound far beyond any progressing run, so it never decides a healthy
//! campaign. What ends a stuck campaign is [`CampaignWatch`]: the campaign records progress on
//! every observed completion or native fact, and fails only after a whole
//! [`STALL`](sugarsubstitute_execution::qualification_time::STALL) of silence.

use std::{
    error::Error,
    fmt,
    sync::Arc,
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::execution::{Clock, Moment, TimeError};
use sugarsubstitute_execution::{
    MonotonicClock,
    qualification_time::{ProgressGuard, STALL, Stall},
};

/// Absolute bound handed to native owners, jobs and child processes that require a deadline.
/// It only stops orphaned work: the campaign's no-progress guard ends a hung run long before.
pub(crate) const CUSTODY: Duration = Duration::from_secs(2 * 60 * 60);

/// Create the real execution clock a campaign's owners run on.
pub(crate) fn execution_clock() -> Arc<MonotonicClock> {
    MonotonicClock::new()
}

/// Sample native time for product budgets and event-loop wakes; it never decides an outcome.
pub(crate) fn native_now() -> Instant {
    Instant::now()
}

/// The campaign's absolute custody bound on both the native and the execution clock.
///
/// The destination epoch is sampled first so sampling latency cannot extend the native bound.
///
/// # Errors
/// Returns [`TimeError::Overflow`] when either clock cannot represent the bound.
pub(crate) fn custody(clock: &dyn Clock) -> Result<(Instant, Moment), TimeError> {
    let execution = clock.now().checked_add(CUSTODY)?;
    let native = native_now().checked_add(CUSTODY).ok_or(TimeError::Overflow)?;
    Ok((native, execution))
}

/// A campaign made no progress for the whole stall interval.
#[derive(Debug)]
pub(crate) struct CampaignStalled {
    campaign: &'static str,
    stall: Stall,
}

impl fmt::Display for CampaignStalled {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} stalled: {}", self.campaign, self.stall)
    }
}

impl Error for CampaignStalled {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.stall)
    }
}

/// Watch one campaign for progress and report a stall exactly once.
pub(crate) struct CampaignWatch {
    campaign: &'static str,
    progress: ProgressGuard,
    reported: bool,
}

impl CampaignWatch {
    /// Start watching `campaign` with the shared stall interval.
    pub(crate) fn new(campaign: &'static str) -> Self {
        Self::with_stall(campaign, STALL)
    }

    fn with_stall(campaign: &'static str, stall: Duration) -> Self {
        let watch = Self { campaign, progress: ProgressGuard::new(stall), reported: false };
        watch.progress.note();
        watch
    }

    /// A watch that has already gone silent, so tests reach the stall path without waiting.
    #[cfg(test)]
    pub(crate) fn silent(campaign: &'static str) -> Self {
        Self::with_stall(campaign, Duration::ZERO)
    }

    /// Record an observed completion, native fact or phase transition.
    pub(crate) fn progressed(&self) {
        self.progress.note();
    }

    /// Fail once when nothing has progressed for the stall interval; later checks pass so the
    /// campaign can still shut down and report.
    ///
    /// # Errors
    /// Returns [`CampaignStalled`] the first time the stall is observed.
    pub(crate) fn check(&mut self) -> Result<(), CampaignStalled> {
        if self.reported {
            return Ok(());
        }
        self.progress.check().map_err(|stall| {
            self.reported = true;
            CampaignStalled { campaign: self.campaign, stall }
        })
    }

    /// Whether the stall has been reported.
    #[cfg(test)]
    pub(crate) fn reported(&self) -> bool {
        self.reported
    }

    /// When the native loop must wake to observe a stall, until it has been reported.
    pub(crate) fn next_deadline(&self) -> Option<Instant> {
        (!self.reported).then(|| self.progress.deadline())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sugarsubstitute_execution::clock_fixture::ManualClock;

    /// A silent campaign fails once, names itself and stops asking the loop to wake.
    #[test]
    fn a_silent_campaign_reports_one_stall() {
        let mut watch = CampaignWatch::silent("Registry qualification");
        assert!(watch.next_deadline().is_some());
        let stalled = watch.check().err().map(|stalled| stalled.to_string()).unwrap_or_default();
        assert!(stalled.starts_with("Registry qualification stalled: nothing made progress"));
        assert!(watch.reported());
        assert!(watch.check().is_ok());
        assert_eq!(watch.next_deadline(), None);
    }

    /// A progressing campaign passes and keeps its wake ahead of the latest progress.
    #[test]
    fn a_progressing_campaign_passes() -> Result<(), CampaignStalled> {
        let mut watch = CampaignWatch::new("Lifecycle qualification");
        watch.progressed();
        watch.check()?;
        assert!(!watch.reported());
        assert!(watch.next_deadline().is_some());
        Ok(())
    }

    /// The custody bound lies the custody interval past the execution clock's current moment.
    #[test]
    fn custody_is_far_on_the_execution_clock() -> Result<(), TimeError> {
        let clock = ManualClock::starting_at(Moment::from_nanos(5));
        let (_, execution) = custody(clock.as_ref())?;
        assert_eq!(execution, Moment::from_nanos(5).checked_add(CUSTODY)?);
        Ok(())
    }
}
