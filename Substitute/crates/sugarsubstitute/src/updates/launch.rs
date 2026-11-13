// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! What one launch does about an application update the coordinator prepares while it runs.
//!
//! Beta-eager builds check as soon as the coordinator can, without waiting for the window. An
//! update prepared before the window opened is started at once: the splash says "Updating to
//! version X…" and the app restarts into it through the normal trial launch from its receipt.
//! The window never waits for the network, and an update prepared after it opened starts on
//! the next launch, so the app never restarts under the user. Standard builds check shortly
//! after the window is ready and apply on the next launch or on "restart to apply".
//!
//! This owner is pure state. The coordinator admits the checks, observes readiness and the
//! check's result, and closes and relaunches when [`LaunchUpdate::restart`] names a version.
use super::{UpdatePolicy, schedule::FIRST_CHECK};
use std::time::Duration;
use sugarsubstitute_contracts::update::attempt::AttemptOutcome;

/// One launch's update handling under the build's compiled policy.
#[derive(Debug)]
pub(crate) struct LaunchUpdate {
    eager: bool,
    window_opened: bool,
    restart: Option<String>,
}
impl LaunchUpdate {
    /// Plan for this build's policy; `None` means the build has no update configuration.
    pub(crate) const fn new(policy: Option<UpdatePolicy>) -> Self {
        Self {
            eager: matches!(policy, Some(UpdatePolicy::BetaEager)),
            window_opened: false,
            restart: None,
        }
    }

    /// Whether a check may begin before the window is ready.
    pub(crate) const fn checks_before_window(&self) -> bool {
        self.eager
    }

    /// Delay from the moment the coordinator may first check to that check.
    pub(crate) const fn first_check(&self) -> Duration {
        if self.eager { Duration::ZERO } else { FIRST_CHECK }
    }

    /// The window is ready; from now on a prepared update waits for the next launch. A restart
    /// already decided is withdrawn, because the app never closes a window the user has seen;
    /// returns whether one was.
    pub(crate) fn window_opened(&mut self) -> bool {
        self.window_opened = true;
        self.restart.take().is_some()
    }

    /// Adopt a finished check. A beta-eager launch whose window has not opened restarts into
    /// the version the check prepared; every other outcome leaves the launch running.
    pub(crate) fn finish(&mut self, outcome: &AttemptOutcome) {
        if let AttemptOutcome::Prepared { version } = outcome
            && self.eager
            && !self.window_opened
            && self.restart.is_none()
        {
            self.restart = Some(version.clone());
        }
    }

    /// The version this launch restarts into, once decided.
    pub(crate) fn restart(&self) -> Option<&str> {
        self.restart.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sugarsubstitute_contracts::update::attempt::{AttemptPhase, UpdateReport};

    fn prepared(version: &str) -> AttemptOutcome {
        AttemptOutcome::Prepared { version: version.into() }
    }

    #[test]
    fn beta_eager_checks_at_once_and_standard_checks_shortly_after_the_window_is_ready() {
        let eager = LaunchUpdate::new(Some(UpdatePolicy::BetaEager));
        assert!(eager.checks_before_window());
        assert_eq!(eager.first_check(), Duration::ZERO);
        for policy in [Some(UpdatePolicy::Standard), None] {
            let launch = LaunchUpdate::new(policy);
            assert!(!launch.checks_before_window());
            assert_eq!(launch.first_check(), FIRST_CHECK);
        }
    }

    #[test]
    fn beta_eager_restarts_into_an_update_prepared_before_the_window_opened() {
        let mut launch = LaunchUpdate::new(Some(UpdatePolicy::BetaEager));
        launch.finish(&AttemptOutcome::Current);
        assert_eq!(launch.restart(), None);
        launch.finish(&prepared("2.0.0"));
        assert_eq!(launch.restart(), Some("2.0.0"));
        launch.finish(&prepared("2.1.0"));
        assert_eq!(launch.restart(), Some("2.0.0"), "the first decision stands");
    }

    #[test]
    fn a_window_that_opens_after_a_restart_was_decided_withdraws_it() {
        let mut launch = LaunchUpdate::new(Some(UpdatePolicy::BetaEager));
        launch.finish(&prepared("2.0.0"));
        assert!(launch.window_opened(), "the decided restart is withdrawn");
        assert_eq!(launch.restart(), None);
        assert!(!launch.window_opened());
        launch.finish(&prepared("2.1.0"));
        assert_eq!(launch.restart(), None);
    }

    #[test]
    fn an_update_prepared_after_the_window_opened_waits_for_the_next_launch() {
        let mut launch = LaunchUpdate::new(Some(UpdatePolicy::BetaEager));
        assert!(!launch.window_opened());
        launch.finish(&prepared("2.0.0"));
        assert_eq!(launch.restart(), None);
    }

    #[test]
    fn standard_builds_and_failed_checks_never_restart() {
        for policy in [Some(UpdatePolicy::Standard), None] {
            let mut launch = LaunchUpdate::new(policy);
            launch.finish(&prepared("2.0.0"));
            assert_eq!(launch.restart(), None);
        }
        let mut eager = LaunchUpdate::new(Some(UpdatePolicy::BetaEager));
        for outcome in [
            AttemptOutcome::Failed(UpdateReport {
                version: Some("2.0.0".into()),
                phase: AttemptPhase::Downloading,
                causes: vec!["Update repository transport failed".into()],
            }),
            AttemptOutcome::NeedsInstaller,
            AttemptOutcome::AlreadyPreparing,
        ] {
            eager.finish(&outcome);
        }
        assert_eq!(eager.restart(), None);
    }
}
