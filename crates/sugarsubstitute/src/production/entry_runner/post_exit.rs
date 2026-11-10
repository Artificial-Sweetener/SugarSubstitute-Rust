// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep only updater observation active after the application has demonstrably retired.
use super::*;
use sugarsubstitute_contracts::execution::Moment;

/// Native wake and executor cutoff for the same immutable post-exit interval.
pub(super) struct Deadline {
    until: Moment,
    pub(super) wake_at: Instant,
}
impl Deadline {
    /// Project only the remaining original interval; observing it late never starts a new interval.
    fn new(until: Moment, now: Moment, anchor: Instant) -> Result<Self, Box<dyn Error>> {
        let remaining = until.checked_duration_since(now).unwrap_or(Duration::ZERO);
        Ok(Self { until, wake_at: anchor.checked_add(remaining).ok_or("Post-exit wake overflow")? })
    }

    /// Early helper exit can shorten cleanup; repeated observations cannot extend it.
    fn tighten(&mut self, until: Moment) -> Result<(), Box<dyn Error>> {
        if until < self.until {
            let reduction = self.until.checked_duration_since(until)?;
            self.wake_at = self.wake_at.checked_sub(reduction).ok_or("Post-exit wake underflow")?;
            self.until = until;
        }
        Ok(())
    }
}

impl<C: EntryActivation> EntryRunner<C> {
    /// Called solely after child/intake/claims/entry settlement and observed installation release.
    /// The original application budget remains unchanged and is never used to authorize helper work.
    pub(super) fn enter_post_exit(&mut self) -> Result<bool, Box<dyn Error>> {
        if !self.normal_shutdown
            || self.failure_source().is_some()
            || self.deadline_reported
            || !self.child_closed
            || !self.intake_settled
            || self.execution_stopping
        {
            return Ok(false);
        }
        let Some(until) =
            self.updater.as_ref().and_then(crate::updates::parent::Parent::post_exit_limit)
        else {
            return Ok(false);
        };
        self.post_exit = Some(Deadline::new(until, self.client()?.now()?, (self.native_now)())?);
        tracing::info!(
            operation = "update_post_exit_started",
            "Application custody retired; bounded update preparation remains"
        );
        Ok(true)
    }

    /// Pump only the independent helper and then join the original executor, without reentering
    /// application lifecycle owners or reacquiring any run/profile/backend/entry claim.
    pub(super) fn poll_post_exit(&mut self) -> Result<bool, Box<dyn Error>> {
        let phase = self.post_exit.as_ref().ok_or("Post-exit phase absent")?;
        if !self.execution_stopping && (self.native_now)() >= phase.wake_at {
            self.execution
                .as_mut()
                .ok_or("Post-exit execution absent")?
                .request_shutdown(phase.until);
            self.execution_stopping = true;
        }
        let drained = self.drain(NonZeroUsize::MIN.saturating_add(31));
        if self.execution_stopping {
            if let Err(cause) = drained {
                self.retain(cause);
            }
            return self.join_post_exit();
        }
        drained?;
        let now = self.client()?.now()?;
        let phase = self.post_exit.as_mut().ok_or("Post-exit phase absent")?;
        if let Some(until) =
            self.updater.as_ref().and_then(crate::updates::parent::Parent::post_exit_limit)
        {
            phase.tighten(until)?;
        }
        let helper_settled =
            self.updater.as_ref().is_none_or(crate::updates::parent::Parent::settled);
        if !helper_settled && now < phase.until {
            return Ok(false);
        }
        self.execution.as_mut().ok_or("Post-exit execution absent")?.request_shutdown(phase.until);
        self.execution_stopping = true;
        self.join_post_exit()
    }

    /// Once closing begins, diagnostic and clock failures cannot veto observation of actual joins.
    fn join_post_exit(&mut self) -> Result<bool, Box<dyn Error>> {
        let helper_settled =
            self.updater.as_ref().is_none_or(crate::updates::parent::Parent::settled);
        if !helper_settled && !self.deadline_reported {
            self.deadline_reported = true;
            return Err("Update helper remains owned after its post-exit cutoff".into());
        }
        let actual =
            self.execution.as_ref().ok_or("Post-exit execution absent")?.poll_shutdown()?;
        if !actual.settled && actual.deadline_expired && !self.deadline_reported {
            self.deadline_reported = true;
            return Err("Post-exit execution remains owned after its cutoff".into());
        }
        Ok(actual.settled)
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{frozen_native_now, settle};
    use super::*;
    use crate::{completion_wake::Wake, updates::test_support::Manual};
    use std::sync::atomic::Ordering;
    type Fixture = (EntryRunner<super::super::tests::NoChild>, Arc<Manual>, Arc<Wake>);
    fn seconds(value: u64) -> Moment {
        Moment::from_nanos(value * 1_000_000_000)
    }

    /// Start real execution without admitting any filesystem or process owner.
    fn runner() -> Result<Fixture, Box<dyn Error>> {
        let root = PathBuf::from(r"E:\unopened-installation");
        let command = ProductionCommand {
            launch_selection: None,
            diagnostics: Default::default(),
            installation: root.clone(),
            options: sugarsubstitute_configuration::paths::StartupPaths::new([
                root.clone(),
                root.clone(),
                root.clone(),
                root.clone(),
                root.clone(),
                root.clone(),
            ])?
            .into(),
        };
        let mut runner = EntryRunner::new(
            command,
            root,
            Box::new(super::super::tests::NoChild(None)),
            crate::diagnostics::DiagnosticHealth::fixture(),
        );
        let clock = Arc::new(Manual::default());
        let wake = Arc::new(Wake::default());
        runner.clock = clock.clone();
        runner.native_now = frozen_native_now;
        runner.initialize_execution(wake.clone())?;
        runner.closing = true;
        runner.normal_shutdown = true;
        runner.shutdown = Some(ShutdownBudget::new(seconds(0))?);
        runner.cleanup_deadline = Some(frozen_native_now());
        Ok((runner, clock, wake))
    }

    #[test]
    fn settled_post_exit_joins_without_reentering_application_cleanup() -> Result<(), Box<dyn Error>>
    {
        let (mut runner, _, wake) = runner()?;
        let original = runner.shutdown;
        runner.post_exit = Some(Deadline::new(seconds(135), seconds(0), frozen_native_now())?);
        let wake_at = runner.post_exit.as_ref().ok_or("Phase absent")?.wake_at;
        assert_eq!(runner.next_deadline(), Some(wake_at));
        settle(&wake, || runner.poll_shutdown())?;
        assert!(!runner.child_closed, "Retired child was polled again");
        assert!(!runner.intake_settled, "Retired intake was polled again");
        assert!(!runner.deadline_reported);
        assert_eq!(runner.shutdown, original);
        runner.finish_shutdown()?;
        assert!(runner.finished);
        Ok(())
    }

    #[test]
    fn late_post_exit_observation_accepts_actual_join_without_renewing_cutoff()
    -> Result<(), Box<dyn Error>> {
        let (mut runner, clock, wake) = runner()?;
        runner.post_exit = Some(Deadline::new(seconds(135), seconds(0), frozen_native_now())?);
        let execution = runner.execution.as_mut().ok_or("Execution absent")?;
        execution.request_shutdown(seconds(135));
        runner.execution_stopping = true;
        settle(&wake, || Ok(execution.poll_shutdown()?.settled))?;
        clock.0.store(200_000_000_000, Ordering::SeqCst);
        assert!(runner.poll_shutdown()?);
        assert!(!runner.deadline_reported);
        assert_eq!(runner.post_exit.as_ref().ok_or("Phase absent")?.until, seconds(135));
        runner.finish_shutdown()?;
        Ok(())
    }

    #[test]
    fn post_exit_cutoff_joins_even_when_completion_observation_is_unavailable()
    -> Result<(), Box<dyn Error>> {
        let (mut runner, _, wake) = runner()?;
        runner.post_exit = Some(Deadline { until: seconds(135), wake_at: frozen_native_now() });
        runner.client = None;
        settle(&wake, || runner.poll_shutdown())?;
        assert!(runner.execution_stopping);
        assert!(runner.failure_source().is_some());
        assert!(!runner.deadline_reported, "A completed actual join is not a timeout");
        assert_eq!(runner.post_exit.as_ref().ok_or("Phase absent")?.until, seconds(135));
        runner.finish_shutdown()?;
        Ok(())
    }

    #[test]
    fn post_exit_projection_uses_remaining_time_and_tightens_without_renewal()
    -> Result<(), Box<dyn Error>> {
        let anchor = frozen_native_now();
        let mut deadline = Deadline::new(seconds(135), seconds(10), anchor)?;
        assert_eq!(deadline.wake_at, anchor + Duration::from_secs(125));
        deadline.tighten(seconds(40))?;
        assert_eq!(deadline.wake_at, anchor + Duration::from_secs(30));
        deadline.tighten(seconds(135))?;
        assert_eq!(deadline.until, seconds(40));
        assert_eq!(deadline.wake_at, anchor + Duration::from_secs(30));
        let expired = Deadline::new(seconds(5), seconds(10), anchor)?;
        assert_eq!(expired.wake_at, anchor);
        Ok(())
    }
}
