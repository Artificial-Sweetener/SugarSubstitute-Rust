// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve the sole execution owner, original budgets and scopes until actual joins.
use super::*;
impl<C: EntryActivation> HeadlessLifetime for EntryRunner<C> {
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<(), Box<dyn Error>> {
        self.initialize_execution(wake)?;
        self.begin_entry()
    }
    fn prepare_native_runtime(
        &mut self,
        max: NonZeroUsize,
    ) -> Result<StartupPreparationStep, Box<dyn Error>> {
        self.drain(max)?;
        Ok(if self.closing {
            StartupPreparationStep::Closing
        } else if self.intake_ready {
            StartupPreparationStep::Ready
        } else {
            StartupPreparationStep::Waiting
        })
    }
    fn activity(&mut self, max: NonZeroUsize) -> Result<HeadlessStep, Box<dyn Error>> {
        self.drain(max)
    }
    fn next_deadline(&self) -> Option<Instant> {
        if let Some(post_exit) = &self.post_exit {
            let helper =
                self.updater.as_ref().and_then(crate::updates::parent::Parent::next_deadline);
            return if self.deadline_reported {
                helper
            } else {
                Some(helper.map_or(post_exit.wake_at, |wake| wake.min(post_exit.wake_at)))
            };
        }
        let own = if self.closing {
            (!self.deadline_reported).then_some(self.cleanup_deadline).flatten()
        } else if !self.initial_completed {
            let forward = self
                .forward_wait
                .as_ref()
                .and_then(forward_recovery::wait::ForwardWait::native_wake);
            self.setup_deadline.into_iter().chain(forward).min()
        } else {
            None
        };
        let child = [
            self.child.next_deadline(),
            self.updater.as_ref().and_then(crate::updates::parent::Parent::next_deadline),
            (!self.closing).then_some(self.update_wake).flatten(),
        ]
        .into_iter()
        .flatten()
        .min();
        match (own, child) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }
    fn request_shutdown(&mut self) -> Result<(), Box<dyn Error>> {
        if self.closing {
            return Ok(());
        }
        self.closing = true;
        let close_requested = close_requested(
            self.normal_shutdown || self.child.normal_close_requested(),
            self.child.desktop_close_intent(),
        );
        if let Some(authority) = &mut self.authority {
            observe_close_cause(authority, close_requested);
        }
        drop(self.pending_activation.take());
        if let Some(bootstrap) = &mut self.bootstrap {
            bootstrap.request_close();
        }
        if let Some(intake) = &mut self.intake {
            intake.close();
        }
        if let Some(forward) = &self.forward {
            forward.control.close();
        }
        let anchor = (self.native_now)();
        self.shutdown = self.child.shutdown_budget();
        let sampled = self.client.as_ref().map(ExecutionClient::now).transpose();
        let mut first: Option<Box<dyn Error>> = None;
        match sampled {
            Ok(Some(now)) => {
                match self.shutdown.map(Ok).unwrap_or_else(|| ShutdownBudget::new(now)) {
                    Ok(original) => {
                        let budget = tighten(original, self.child.shutdown_budget());
                        self.shutdown = Some(budget);
                        let remaining = budget
                            .settle_until
                            .checked_duration_since(now)
                            .unwrap_or(Duration::ZERO);
                        self.cleanup_deadline = anchor.checked_add(remaining);
                    }
                    Err(cause) => first = Some(Box::new(cause)),
                }
            }
            Ok(None) => {}
            Err(cause) => {
                first = Some(Box::new(cause) as Box<dyn Error>);
                self.shutdown = self.child.shutdown_budget();
            }
        }
        // Native close intent is issued even if the actual clock observation failed.
        if let Err(cause) = self.child.request_close(self.shutdown) {
            if first.is_none() {
                first = Some(cause);
            } else {
                self.retain(cause);
            }
        }
        if let Some(cause) = first {
            return Err(cause);
        }
        Ok(())
    }
    fn take_presentation(&mut self) -> Option<sugarsubstitute_desktop::HeadlessPresentation> {
        self.take_forward_presentation()
    }
    fn poll_shutdown(&mut self) -> Result<bool, Box<dyn Error>> {
        if !self.closing {
            return Ok(false);
        }
        if self.execution.is_none() {
            return Ok(true);
        }
        if self.post_exit.is_some() {
            return self.poll_post_exit();
        }
        if let Some(original) = self.shutdown {
            let tightened = tighten(original, self.child.shutdown_budget());
            if tightened.settle_until < original.settle_until
                && let Some(deadline) = self.cleanup_deadline
            {
                let reduction =
                    original.settle_until.checked_duration_since(tightened.settle_until)?;
                self.cleanup_deadline = deadline.checked_sub(reduction);
            }
            self.shutdown = Some(tightened);
        }
        let native_now = (self.native_now)();
        let cutoff = self.consume_cleanup_cutoff(native_now);
        if cutoff {
            let budget = self.shutdown.ok_or("Original production shutdown budget missing")?;
            if let Some(execution) = &mut self.execution
                && !self.execution_stopping
            {
                self.execution_stopping = true;
                execution.request_shutdown(budget.settle_until);
            }
        }
        let drained = self.drain(NonZeroUsize::MIN.saturating_add(31));
        if self.deadline_reported && self.execution_stopping {
            if let Err(cause) = drained {
                self.retain(cause);
            }
        } else {
            drained?;
        }
        if cutoff {
            return Err("Production shutdown remains owned after its original cutoff".into());
        }
        if self.deadline_reported && self.execution_stopping {
            // Logical cleanup can retain a failure after every actual job and worker
            // has stopped. Let the native window retire and return that failure;
            // waiting for a failed logical owner here would hide the process forever.
            let actual =
                self.execution.as_ref().ok_or("Production execution missing")?.poll_shutdown()?;
            let now = (self.native_now)();
            if self
                .shutdown_diagnostic
                .is_none_or(|previous| now.duration_since(previous) >= Duration::from_secs(5))
            {
                tracing::warn!(operation="production_retained_shutdown", progress=?actual,
                    "Original shutdown is still observing actual execution retirement");
                self.shutdown_diagnostic = Some(now);
            }
            return Ok(actual.settled);
        }
        if !self.child_closed || !self.intake_settled {
            return Ok(false);
        }
        if let Some(authority) = &mut self.authority {
            authority.request_close();
        }
        let budget = self.shutdown.ok_or("Original production shutdown budget missing")?;
        let client = self.client.as_ref().ok_or("Production cleanup client missing")?;
        let cleanup = self.cleanup.as_ref().ok_or("Production cleanup scope missing")?;
        if let Some(authority) = &mut self.authority {
            match authority.submit_close(client, cleanup, &self.jobs, budget.settle_until) {
                Ok(()) | Err(super::super::coordinator_authority::AuthorityIssue::Pending) => {}
                Err(cause) => return Err(Box::new(cause)),
            }
            if !authority.settled() {
                return Ok(false);
            }
        }
        if let Some(bootstrap) = &mut self.bootstrap
            && !bootstrap.retire(cleanup, &self.jobs, budget.settle_until)?
        {
            return Ok(false);
        }
        if self.updater.as_ref().is_some_and(crate::updates::parent::Parent::borrows_application) {
            return Ok(false);
        }
        if let Some(authority) = &mut self.authority
            && !authority.retire_installation(client, cleanup, &self.jobs)?
        {
            return Ok(false);
        }
        if self.updater.as_ref().is_some_and(|updater| !updater.settled()) {
            if self.enter_post_exit()? {
                return self.poll_post_exit();
            }
            return Ok(false);
        }
        let execution = self.execution.as_mut().ok_or("Production execution missing")?;
        if !self.execution_stopping {
            self.execution_stopping = true;
            execution.request_shutdown(budget.settle_until);
        }
        let actual = execution.poll_shutdown()?;
        if actual.deadline_expired && !self.deadline_reported {
            self.deadline_reported = true;
            return Err("Production shutdown remains owned after its original cutoff".into());
        }
        Ok(actual.settled)
    }
    fn session_ending(&mut self) {
        let Some(authority) = &mut self.authority else { return };
        let (Some(client), Some(scope)) = (&self.client, &self.finite) else {
            authority.observe_trial_close(
                sugarsubstitute_contracts::installation::CloseCause::SessionEnd,
            );
            return;
        };
        if let Err(cause) = authority.observe_session_end(client, scope, &self.jobs) {
            tracing::warn!(operation = "production_session_end", cause = %cause,
                "The session end could not settle the update trial before the close save");
        }
    }
    fn finish_shutdown(&mut self) -> Result<(), Box<dyn Error>> {
        if self.finished {
            return Err("Production lifetime already finished".into());
        }
        if !self.poll_shutdown()? {
            return Err("Production lifetime has unsettled actual resources".into());
        }
        if let Some(execution) = &mut self.execution {
            execution.finish_shutdown()?;
        }
        if self.deadline_reported {
            return Err(
                "Production shutdown exceeded its deadline; execution workers joined".into()
            );
        }
        self.child.finish()?;
        // Original scopes, bootstrap outcomes and causes remain in this owner
        // through the real join; any returned RunFailure retains them afterwards.
        self.execution = None;
        self.finished = true;
        Ok(())
    }
}
impl<C: EntryActivation> EntryRunner<C> {
    /// Consume the already-issued native wake cutoff once, without treating it as
    /// native retirement. Removing a past wake keeps uncertain custody parked on
    /// actual completion wakes instead of an immediately expiring native wait.
    pub(super) fn consume_cleanup_cutoff(&mut self, now: Instant) -> bool {
        if self.closing
            && !self.deadline_reported
            && self.cleanup_deadline.is_some_and(|until| now >= until)
        {
            self.deadline_reported = true;
            true
        } else {
            false
        }
    }
}
fn tighten(original: ShutdownBudget, previous: Option<ShutdownBudget>) -> ShutdownBudget {
    previous.map_or(original, |previous| ShutdownBudget {
        started: original.started.min(previous.started),
        graceful_until: original.graceful_until.min(previous.graceful_until),
        settle_until: original.settle_until.min(previous.settle_until),
    })
}

/// Whether this run was asked to close: the user's or the app's own close, or the desktop's
/// authenticated exit intent attributing its exit to a deliberate close. A desktop closing for
/// a session end announces a deliberate close too, so that exit is a close in whichever order
/// it and this process's own session-end signal arrive. A desktop that announces a startup
/// failure, or that ends without announcing anything, was not asked to close.
fn close_requested(
    requested: bool,
    desktop: Option<sugarsubstitute_contracts::lifecycle::CompletionIntent>,
) -> bool {
    use sugarsubstitute_contracts::lifecycle::CompletionIntent;
    requested
        || matches!(desktop, Some(CompletionIntent::Normal | CompletionIntent::StartupCancelled))
}

/// Record why this run is closing, for an unaccepted update trial's verdict. A close the user
/// or the app asked for is no verdict. A run ending without any close request, such as its
/// window exiting on its own before readiness, is the host failing its own startup. A failure
/// or session end observed earlier decides first.
fn observe_close_cause(authority: &mut CoordinatorAuthority, close_requested: bool) {
    if close_requested {
        authority
            .observe_trial_close(sugarsubstitute_contracts::installation::CloseCause::Requested);
    } else {
        authority
            .observe_trial_failure(sugarsubstitute_contracts::installation::FailedProcess::Host);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use sugarsubstitute_contracts::installation::{BadTrial, CloseCause, TrialVerdict, Undecided};

    fn authority() -> Result<CoordinatorAuthority, Box<dyn Error>> {
        Ok(CoordinatorAuthority::new(
            sugarsubstitute_contracts::coordinator_entry::EntrySetupBudget::new(
                sugarsubstitute_contracts::execution::Moment::from_nanos(0),
            )?,
            None,
        )?)
    }

    /// A trial whose window exits on its own before readiness is the release's own startup
    /// failure, so it is struck; closing during a slow first start, or a session end, is not.
    #[test]
    fn only_a_run_ending_without_a_close_request_judges_the_trial_bad() -> Result<(), Box<dyn Error>>
    {
        let mut crashed = authority()?;
        observe_close_cause(&mut crashed, false);
        assert_eq!(crashed.trial_verdict(), TrialVerdict::Bad(BadTrial::StartupFailed));
        let mut closed = authority()?;
        observe_close_cause(&mut closed, true);
        assert_eq!(closed.trial_verdict(), TrialVerdict::NoVerdict(Undecided::CloseRequested));
        let mut ending = authority()?;
        ending.observe_trial_close(CloseCause::SessionEnd);
        observe_close_cause(&mut ending, false);
        assert_eq!(ending.trial_verdict(), TrialVerdict::NoVerdict(Undecided::SessionEnd));
        Ok(())
    }

    /// The desktop's session-end close announces a deliberate exit. When it reaches this run
    /// before this process's own session-end signal, the trial is still no verdict; a desktop
    /// that announces a startup failure, or none at all, failed its own start.
    #[test]
    fn a_desktop_session_end_exit_is_no_verdict_whichever_signal_arrives_first()
    -> Result<(), Box<dyn Error>> {
        use sugarsubstitute_contracts::lifecycle::CompletionIntent;
        for intent in [CompletionIntent::Normal, CompletionIntent::StartupCancelled] {
            let mut reversed = authority()?;
            observe_close_cause(&mut reversed, close_requested(false, Some(intent)));
            reversed.observe_trial_close(CloseCause::SessionEnd);
            assert_eq!(
                reversed.trial_verdict(),
                TrialVerdict::NoVerdict(Undecided::CloseRequested)
            );
        }
        for desktop in [Some(CompletionIntent::StartupFailed), None] {
            let mut failed = authority()?;
            observe_close_cause(&mut failed, close_requested(false, desktop));
            assert_eq!(failed.trial_verdict(), TrialVerdict::Bad(BadTrial::StartupFailed));
        }
        Ok(())
    }
    use sugarsubstitute_contracts::execution::Moment;
    #[test]
    fn actual_child_cutoffs_tighten_both_intervals_without_renewal() -> Result<(), Box<dyn Error>> {
        let earlier = ShutdownBudget::new(Moment::from_nanos(0))?;
        let later = ShutdownBudget::new(Moment::from_nanos(1_000))?;
        assert_eq!(tighten(later, Some(earlier)), earlier);
        assert_eq!(tighten(earlier, Some(later)), earlier);
        Ok(())
    }
}
