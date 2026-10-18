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
        let own = if self.closing {
            (!self.deadline_reported).then_some(self.cleanup_deadline).flatten()
        } else if !self.initial_completed {
            self.setup_deadline
        } else {
            None
        };
        match (own, self.child.next_deadline()) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }
    fn request_shutdown(&mut self) -> Result<(), Box<dyn Error>> {
        if self.closing {
            return Ok(());
        }
        self.closing = true;
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
        let anchor = Instant::now();
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
    fn poll_shutdown(&mut self) -> Result<bool, Box<dyn Error>> {
        if !self.closing {
            return Ok(false);
        }
        if self.execution.is_none() {
            return Ok(true);
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
        let cutoff = self.consume_cleanup_cutoff(Instant::now());
        if cutoff {
            let budget = self.shutdown.ok_or("Original production shutdown budget missing")?;
            if let Some(execution) = &mut self.execution
                && !self.execution_stopping
            {
                self.execution_stopping = true;
                execution.request_shutdown(budget.settle_until);
            }
        }
        self.drain(NonZeroUsize::MIN.saturating_add(31))?;
        if cutoff {
            return Err("Production shutdown remains owned after its original cutoff".into());
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
        if let Some(authority) = &mut self.authority
            && !authority.settled()
        {
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
    /// actual completion wakes and the desktop owner's existing closing cadence.
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
#[cfg(test)]
mod tests {
    use super::*;
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
