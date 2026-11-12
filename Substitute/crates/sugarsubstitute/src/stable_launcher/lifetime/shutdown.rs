// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep launcher cleanup under one cutoff and retain rejected or failed creator retirement.
use super::*;
use sugarsubstitute_contracts::lifecycle::ShutdownBudget;

impl Launcher {
    /// Freeze cleanup at the first close; repeated close cannot cancel an admitted cleanup job.
    pub(super) fn begin_close(&mut self) -> Result<()> {
        if self.closing {
            return Ok(());
        }
        let now = self.clock.now();
        let budget = ShutdownBudget::new(now)?;
        let until = [
            Some(budget.settle_until),
            self.recovery_until,
            self.cached.as_ref().and_then(cached::Cached::cleanup_until),
        ]
        .into_iter()
        .flatten()
        .min()
        .ok_or("Launcher cleanup cutoff absent")?;
        self.cleanup_until = Some(until);
        self.wake_at =
            Instant::now().checked_add(until.checked_duration_since(now).unwrap_or_default());
        self.closing = true;
        self.activation_after = None;
        self.activation_wake = None;
        if let Some(job) = &self.job {
            job.job.cancel();
        }
        Ok(())
    }

    /// Rejected admission leaves creator custody available for cleanup on the next pump turn.
    /// At the original cutoff, observe actual executor joins even if completion routing fails.
    pub(super) fn poll_close(&mut self) -> Result<bool> {
        if !self.closing {
            return Ok(false);
        }
        let until = self.cleanup_until.ok_or("Launcher cleanup cutoff absent")?;
        let expired = self.clock.now() >= until;
        if expired && !self.stopping {
            self.stop_execution(until);
        }
        let advanced = self.advance(NonZeroUsize::MIN.saturating_add(31));
        if self.stopping {
            if let Err(cause) = advanced {
                self.failure.get_or_insert(cause);
            }
            return self.observe_join(expired);
        }
        advanced?;
        if self.job.is_some() || self.cached.is_some() {
            return Ok(false);
        }
        if self.activation.is_some() {
            if self.activation_due() {
                self.observe_activation(true)?;
            }
            return Ok(false);
        }
        if self.launch.is_some() || self.engine.is_some() || self.source.is_some() {
            if !self.activation_due() {
                return Ok(false);
            }
            let launch = self.launch.clone();
            let engine = self.engine.clone();
            let source = self.source.clone();
            self.submit(Task::Retire, move |context| {
                if let Some(source) = source
                    && !source.retire(&context)?
                {
                    return Ok(Work::Retiring);
                }
                if let Some(launch) = launch {
                    launch.recover_and_retire(&context).map_err(io::Error::other)?;
                }
                if let Some(engine) = engine {
                    engine.retire();
                }
                Ok(Work::Retired)
            })?;
            return Ok(false);
        }
        self.stop_execution(until);
        self.observe_join(false)
    }

    /// Executor cancellation uses the already-issued cleanup cutoff, including when observed late.
    fn stop_execution(&mut self, until: Moment) {
        if let Some(owner) = &mut self.owner {
            owner.request_shutdown(until);
        }
        self.stopping = true;
    }

    /// Logical retirement and actual worker joins are separate facts; report unsettled custody once.
    fn observe_join(&mut self, expired: bool) -> Result<bool> {
        let actual = self.owner.as_ref().map(ExecutionOwner::poll_shutdown).transpose()?;
        let settled = actual.as_ref().is_none_or(|progress| progress.settled);
        let uncertain = self.job.is_some()
            || self.launch.is_some()
            || self.cached.is_some()
            || self.activation.is_some()
            || self.engine.is_some();
        let uncertain = uncertain || self.source.is_some();
        if !self.cutoff_reported
            && ((expired && uncertain)
                || (!settled && actual.is_some_and(|progress| progress.deadline_expired)))
        {
            self.cutoff_reported = true;
            return Err("Launcher ownership remains unsettled after its original cutoff".into());
        }
        Ok(settled)
    }
}
