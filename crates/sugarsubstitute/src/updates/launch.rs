// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Launch-time update decision for the stable launcher: whether to prepare before opening, what
//! the splash shows meanwhile, and which report a failed attempt files.
//!
//! This owner is pure state. The launcher runs the preparation helper, forwards its typed
//! progress and result here, and afterwards starts the selected version through its existing
//! activation path, which picks up a version this attempt prepared. Every failure starts the
//! current version; none blocks the launch.
use super::UpdatePolicy;
use sugarsubstitute_contracts::update::attempt::{
    AttemptOutcome, AttemptPhase, AttemptProgress, UpdateReport,
};

/// What the launcher does about application updates before starting the app.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LaunchPlan {
    /// Start the selected version, activating an update that is already prepared.
    ActivatePrepared,
    /// Run one preparation attempt first, showing its progress, then activate what it prepared
    /// or start the current version when it fails.
    PrepareBeforeOpening,
}
impl LaunchPlan {
    /// Plan for a build's compiled policy; a build without update configuration never prepares.
    pub const fn for_policy(policy: Option<UpdatePolicy>) -> Self {
        match policy {
            Some(UpdatePolicy::BetaEager) => Self::PrepareBeforeOpening,
            Some(UpdatePolicy::Standard) | None => Self::ActivatePrepared,
        }
    }
}

/// The launcher's next step once the attempt, if any, has ended.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LaunchDecision {
    /// Start the selected version. `report` is present when an attempt failed; the launcher
    /// shows one line and files it, and the current version starts.
    StartSelected {
        /// Report of a failed attempt, for the update report surface.
        report: Option<UpdateReport>,
    },
}

#[derive(Debug)]
enum State {
    Planned,
    Preparing { progress: AttemptProgress },
    Decided(LaunchDecision),
}

/// One launch's update handling under the build's compiled policy.
#[derive(Debug)]
pub struct LaunchUpdate {
    plan: LaunchPlan,
    state: State,
}
impl LaunchUpdate {
    /// Plan for this build's policy; `None` means the build has no update configuration.
    pub fn new(policy: Option<UpdatePolicy>) -> Self {
        let plan = LaunchPlan::for_policy(policy);
        let state = match plan {
            LaunchPlan::ActivatePrepared => {
                State::Decided(LaunchDecision::StartSelected { report: None })
            }
            LaunchPlan::PrepareBeforeOpening => State::Planned,
        };
        Self { plan, state }
    }

    /// Whether the launcher must run one preparation attempt before starting the app.
    pub fn prepares_before_opening(&self) -> bool {
        self.plan == LaunchPlan::PrepareBeforeOpening
    }

    /// Record that the helper was admitted; the splash shows the opening phase until progress.
    /// # Errors
    /// Rejects a second attempt, or an attempt under a policy that does not prepare at launch.
    pub fn begin(&mut self) -> Result<(), LaunchUpdateError> {
        if !matches!(self.state, State::Planned) {
            return Err(LaunchUpdateError::Unexpected);
        }
        self.state = State::Preparing {
            progress: AttemptProgress { phase: AttemptPhase::Opening, bytes: None },
        };
        Ok(())
    }

    /// The helper could not be started at all; the current version starts with a report.
    /// # Errors
    /// Rejects an admission failure outside a planned attempt.
    pub fn not_started(
        &mut self,
        cause: &(dyn std::error::Error + 'static),
    ) -> Result<LaunchDecision, LaunchUpdateError> {
        if !matches!(self.state, State::Planned) {
            return Err(LaunchUpdateError::Unexpected);
        }
        let report = UpdateReport::from_error(None, AttemptPhase::Opening, cause);
        Ok(self.decide(Some(report)))
    }

    /// Adopt live progress for the splash. Progress after the decision is ignored as stale.
    /// # Errors
    /// Rejects progress before the attempt began.
    pub fn observe(&mut self, progress: AttemptProgress) -> Result<(), LaunchUpdateError> {
        match &mut self.state {
            State::Preparing { progress: current } => {
                *current = progress;
                Ok(())
            }
            State::Decided(_) => Ok(()),
            State::Planned => Err(LaunchUpdateError::Unexpected),
        }
    }

    /// Conclude from the attempt's typed result, observed after the helper exited. A helper that
    /// exited without a result is a failed attempt at its last reported phase.
    /// # Errors
    /// Rejects a result for an attempt that never began.
    pub fn finish(
        &mut self,
        outcome: Option<AttemptOutcome>,
    ) -> Result<LaunchDecision, LaunchUpdateError> {
        let State::Preparing { progress } = &self.state else {
            return Err(LaunchUpdateError::Unexpected);
        };
        let report = match outcome {
            Some(AttemptOutcome::Failed(report)) => Some(report),
            Some(
                AttemptOutcome::Current
                | AttemptOutcome::Prepared { .. }
                | AttemptOutcome::NeedsInstaller
                | AttemptOutcome::AlreadyPreparing,
            ) => None,
            None => Some(UpdateReport::from_error(None, progress.phase, &MissingResult)),
        };
        Ok(self.decide(report))
    }

    /// What the splash shows while preparing; `None` once decided or when nothing runs.
    pub fn splash(&self) -> Option<AttemptProgress> {
        match self.state {
            State::Preparing { progress } => Some(progress),
            State::Planned | State::Decided(_) => None,
        }
    }

    /// The decision, once reached.
    pub fn decision(&self) -> Option<&LaunchDecision> {
        match &self.state {
            State::Decided(decision) => Some(decision),
            State::Planned | State::Preparing { .. } => None,
        }
    }

    fn decide(&mut self, report: Option<UpdateReport>) -> LaunchDecision {
        let decision = LaunchDecision::StartSelected { report };
        self.state = State::Decided(decision.clone());
        decision
    }
}

/// A launcher call arrived in the wrong order; the launch itself proceeds unaffected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LaunchUpdateError {
    /// The call does not match the attempt's current state.
    Unexpected,
}
impl std::fmt::Display for LaunchUpdateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Launch update call does not match the attempt's state")
    }
}
impl std::error::Error for LaunchUpdateError {}

/// The helper exited without its final typed result.
#[derive(Debug)]
struct MissingResult;
impl std::fmt::Display for MissingResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("The update helper ended without reporting a result")
    }
}
impl std::error::Error for MissingResult {}

#[cfg(test)]
mod tests {
    use super::*;
    type Result = std::result::Result<(), LaunchUpdateError>;

    fn downloading(received: u64) -> AttemptProgress {
        AttemptProgress { phase: AttemptPhase::Downloading, bytes: Some((received, 100)) }
    }

    #[test]
    fn only_beta_eager_builds_prepare_before_opening() {
        assert_eq!(
            LaunchPlan::for_policy(Some(UpdatePolicy::BetaEager)),
            LaunchPlan::PrepareBeforeOpening
        );
        assert_eq!(
            LaunchPlan::for_policy(Some(UpdatePolicy::Standard)),
            LaunchPlan::ActivatePrepared
        );
        assert_eq!(LaunchPlan::for_policy(None), LaunchPlan::ActivatePrepared);
    }

    #[test]
    fn standard_and_unconfigured_builds_start_at_once_without_an_attempt() {
        for policy in [Some(UpdatePolicy::Standard), None] {
            let launch = LaunchUpdate::new(policy);
            assert!(!launch.prepares_before_opening());
            assert_eq!(launch.splash(), None);
            assert_eq!(launch.decision(), Some(&LaunchDecision::StartSelected { report: None }));
        }
        assert_eq!(
            LaunchUpdate::new(Some(UpdatePolicy::Standard)).begin(),
            Err(LaunchUpdateError::Unexpected)
        );
    }

    #[test]
    fn beta_eager_prepares_first_and_the_splash_follows_progress() -> Result {
        let mut launch = LaunchUpdate::new(Some(UpdatePolicy::BetaEager));
        assert!(launch.prepares_before_opening());
        assert_eq!(launch.decision(), None);
        launch.begin()?;
        assert_eq!(launch.splash().map(|progress| progress.phase), Some(AttemptPhase::Opening));
        launch.observe(downloading(40))?;
        assert_eq!(launch.splash(), Some(downloading(40)));
        let decision = launch.finish(Some(AttemptOutcome::Prepared { version: "2.0.0".into() }))?;
        assert_eq!(decision, LaunchDecision::StartSelected { report: None });
        assert_eq!(launch.splash(), None);
        launch.observe(downloading(90))?;
        assert_eq!(launch.splash(), None);
        Ok(())
    }

    #[test]
    fn a_failed_attempt_starts_the_current_version_with_its_report() -> Result {
        let mut launch = LaunchUpdate::new(Some(UpdatePolicy::BetaEager));
        launch.begin()?;
        let report = UpdateReport {
            version: Some("2.0.0".into()),
            phase: AttemptPhase::Downloading,
            causes: vec!["Update repository transport failed".into()],
        };
        let decision = launch.finish(Some(AttemptOutcome::Failed(report.clone())))?;
        assert_eq!(decision, LaunchDecision::StartSelected { report: Some(report) });
        Ok(())
    }

    #[test]
    fn a_helper_without_a_result_fails_at_its_last_phase() -> Result {
        let mut launch = LaunchUpdate::new(Some(UpdatePolicy::BetaEager));
        launch.begin()?;
        launch.observe(downloading(10))?;
        let LaunchDecision::StartSelected { report: Some(report) } = launch.finish(None)? else {
            return Err(LaunchUpdateError::Unexpected);
        };
        assert_eq!(report.phase, AttemptPhase::Downloading);
        assert_eq!(report.causes, ["The update helper ended without reporting a result"]);
        assert_eq!(launch.finish(None), Err(LaunchUpdateError::Unexpected));
        Ok(())
    }

    #[test]
    fn a_helper_that_cannot_start_still_opens_the_app_with_a_report() -> Result {
        let mut launch = LaunchUpdate::new(Some(UpdatePolicy::BetaEager));
        let cause = std::io::Error::other("Update helper admission failed");
        let LaunchDecision::StartSelected { report: Some(report) } = launch.not_started(&cause)?
        else {
            return Err(LaunchUpdateError::Unexpected);
        };
        assert_eq!(report.phase, AttemptPhase::Opening);
        assert_eq!(report.causes, ["Update helper admission failed"]);
        Ok(())
    }
}
