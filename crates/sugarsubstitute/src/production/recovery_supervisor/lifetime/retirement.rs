// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep fresh handler protection through capture, then observe original containment owners.
use super::*;
use sugarsubstitute_contracts::lifecycle::ShutdownBudget;
impl Lifetime {
    pub(super) fn begin_shutdown(&mut self) -> Result<()> {
        if self.closing {
            return Ok(());
        }
        self.closing = true;
        self.setup_wake = None;
        self.enrollment = None;
        self.startup = None;
        if let Some(channel) = &mut self.channel {
            channel.request_close();
        }
        if let Some(capture) = &mut self.capture {
            capture.request_close();
        }
        if let Some(child) = &self.child {
            child.cancel_creation();
        }
        self.child_input = None;
        let Some(client) = &self.client else {
            return Ok(());
        };
        let now = client.now()?;
        if !self.cleanup.requested() {
            if self.capture.as_ref().is_some_and(Capture::attempted) {
                self.cleanup.capture(now)?;
            } else {
                self.cleanup.root_stop(now, ShutdownBudget::new(now)?.settle_until)?;
            }
        }
        self.refresh_cleanup_wake()?;
        Ok(())
    }
    fn refresh_cleanup_wake(&mut self) -> Result<()> {
        self.cleanup_wake =
            self.cleanup.until().map(|until| self.clock.instant_for(until)).transpose()?;
        Ok(())
    }
    pub(super) fn retire(&mut self) -> Result<bool> {
        let Some(client) = &self.client else {
            return Ok(self.execution.is_none());
        };
        let now = client.now()?;
        let budget = self.cleanup.advance(now)?;
        self.refresh_cleanup_wake()?;
        let Some(budget) = budget else {
            return Ok(false);
        };
        let client = self.client.as_ref().ok_or("Recovery execution absent")?;
        let Some(cleanup) = self.cleanup_scope.as_ref() else {
            if self.channel.is_some() || self.capture.is_some() || self.child.is_some() {
                return Err("Actual recovery resources have no original cleanup scope".into());
            }
            // Scope setup can reject before any inherited/native input is adopted.
            if let Some(execution) = &mut self.execution {
                self.execution_stopping = true;
                execution.request_shutdown(budget.settle_until);
                return Ok(execution.poll_shutdown()?.settled);
            }
            return Ok(true);
        };
        let mut failure = None;
        if self.child.is_none()
            && self.origin.is_some()
            && let Some(job) = self.role.job.take()
        {
            let origin = self.origin.as_ref().ok_or("Authenticated cleanup source absent")?;
            let spec = JobSpec {
                id: self.jobs.allocate()?,
                operation: Operation::NativeLifecycle,
                class: JobClass::InfrastructureLifecycle,
                deadline: Some(budget.settle_until),
                reservation_bytes: 4096,
                replacement: None,
            };
            match job.submit_retirement(client, cleanup, spec, origin) {
                Ok(owner) => {
                    self.child = Some(owner);
                    self.child_cleanup_submitted = true;
                }
                Err(rejected) => {
                    self.child = Some(rejected.owner);
                    retain(&mut failure, Err(Box::new(rejected.cause)));
                }
            }
        }
        if let Some(child) = &mut self.child {
            child.cancel_creation();
            if !self.child_cleanup_submitted && child.unstarted_cleanup_required() {
                let spec = JobSpec {
                    id: self.jobs.allocate()?,
                    operation: Operation::NativeLifecycle,
                    class: JobClass::InfrastructureLifecycle,
                    deadline: Some(budget.settle_until),
                    reservation_bytes: 4096,
                    replacement: None,
                };
                match child.submit_unstarted_cleanup(client, cleanup, spec) {
                    Ok(()) => self.child_cleanup_submitted = true,
                    Err(RegistryAdmissionError::Busy) => {}
                    Err(cause) => retain(&mut failure, Err(Box::new(cause))),
                }
            }
        }
        let capture_safe =
            self.child.as_ref().is_none_or(RecoveryChildCreation::capture_retirement_ready);
        let capture_settled = if capture_safe {
            match &mut self.capture {
                Some(capture) => match capture.close(client, cleanup, &self.jobs, budget) {
                    Ok(settled) => settled,
                    Err(cause) => {
                        retain(&mut failure, Err(cause));
                        false
                    }
                },
                None => true,
            }
        } else {
            false
        };
        let child_settled = self.child.as_ref().is_none_or(RecoveryChildCreation::settled);
        let channel_settled = self.channel.as_ref().is_none_or(Channel::settled);
        let local_settled = capture_settled && child_settled && channel_settled;
        if (local_settled || now >= budget.settle_until)
            && !self.execution_stopping
            && let Some(execution) = &mut self.execution
        {
            self.execution_stopping = true;
            execution.request_shutdown(budget.settle_until);
        }
        let execution_settled = if self.execution_stopping {
            match self.execution.as_ref().map(ExecutionOwner::poll_shutdown).transpose() {
                Ok(progress) => progress.is_none_or(|progress| progress.settled),
                Err(cause) => {
                    retain(&mut failure, Err(Box::new(cause)));
                    false
                }
            }
        } else {
            self.execution.is_none()
        };
        if now >= budget.settle_until {
            self.cleanup_wake = None;
        }
        let settled = capture_settled && child_settled && channel_settled && execution_settled;
        failure.map_or(Ok(settled), Err)
    }
}
