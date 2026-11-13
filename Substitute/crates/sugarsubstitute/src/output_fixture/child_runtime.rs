// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep the native pump alive through actual writes, pipe retirement and worker joins.
use super::{Result, channel::Channel, child::Child};
use std::{num::NonZeroUsize, sync::Arc, time::Instant};
use sugarsubstitute_contracts::execution::{
    Clock, CompletionWake, ExecutionPort, JobClass, JobId, JobSpec, Operation, ScopeId, ScopeSpec,
};
use sugarsubstitute_desktop::{HeadlessLifetime, HeadlessStep};
use sugarsubstitute_execution::{ExecutionOwner, windows_lifecycle_pipe::MIN_LIFECYCLE_PIPE_BYTES};
impl HeadlessLifetime for Child {
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<()> {
        if self.owner.is_some() || self.closing {
            return Err("Output child started twice/closed".into());
        }
        let owner = ExecutionOwner::new(self.clock.clone(), wake)?;
        let client = owner.client();
        self.scope =
            Some(client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?);
        self.owner = Some(owner);
        self.client = Some(client.clone());
        if self.options.role.authenticated() {
            let scope = client
                .create_native_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
            let permit = client.reserve_lifecycle_pipe_client(
                &scope,
                JobSpec {
                    id: JobId::new(1)?,
                    operation: Operation::NativeLifecycle,
                    class: JobClass::NativeObservation,
                    deadline: None,
                    reservation_bytes: MIN_LIFECYCLE_PIPE_BYTES,
                    replacement: None,
                },
                self.options.deadline.ok_or("Output child original QPC deadline missing")?,
            )?;
            self.deadline = Some(permit.original_deadline());
            let session = permit
                .bind(self.options.section.take().ok_or("Output bootstrap already transferred")?)
                .ok_or("Output client permit was consumed")?;
            self.native_scope = Some(scope);
            self.channel = Some(Channel::new(session, self.options.role, None));
        } else {
            self.deadline = Some(
                sugarsubstitute_contracts::lifecycle::ShutdownBudget::new(self.clock.now())?
                    .settle_until,
            );
            self.start_writers()?;
        }
        let wall = crate::campaign_watch::native_now();
        self.deadline_wall = Some(
            wall.checked_add(
                self.deadline
                    .ok_or("Output child deadline missing")?
                    .checked_duration_since(self.clock.now())?,
            )
            .ok_or("Output child wake deadline overflow")?,
        );
        Ok(())
    }
    fn prepare_native_runtime(
        &mut self,
        max: NonZeroUsize,
    ) -> Result<sugarsubstitute_desktop::StartupPreparationStep> {
        if max.get() > 32 {
            return Err("Fixture preparation exceeds native drain bound".into());
        }
        if self.closing {
            return Ok(sugarsubstitute_desktop::StartupPreparationStep::Closing);
        }
        if self.owner.is_none()
            || self.client.is_none()
            || self.scope.is_none()
            || self.deadline.is_none()
            || (self.options.role.authenticated() && self.channel.is_none())
        {
            return Err("Fixture original execution/native admission is incomplete".into());
        }
        Ok(sugarsubstitute_desktop::StartupPreparationStep::Ready)
    }
    fn activity(&mut self, max: NonZeroUsize) -> Result<HeadlessStep> {
        let closing = Child::activity(self, max)?;
        self.retry_retirement()?;
        Ok(if closing { HeadlessStep::Shutdown } else { HeadlessStep::Active })
    }
    fn next_deadline(&self) -> Option<Instant> {
        Child::next_deadline(self)
    }
    fn request_shutdown(&mut self) -> Result<()> {
        if self.closing {
            return Ok(());
        }
        self.closing = true;
        self.retirement_requested = true;
        if let Some(channel) = &mut self.channel {
            channel.close();
        }
        for job in self.writes.iter().flatten() {
            job.cancel();
        }
        self.retry_retirement()?;
        Ok(())
    }
    fn poll_shutdown(&mut self) -> Result<bool> {
        if !self.closing
            || !self.writes_settled()
            || self.channel.as_ref().is_some_and(|c| !c.terminal)
        {
            return Ok(false);
        }
        let Some(owner) = &mut self.owner else { return Ok(true) };
        if !self.execution_stopping {
            self.execution_stopping = true;
            owner.request_shutdown(self.deadline.unwrap_or_else(|| self.clock.now()));
        }
        let progress = owner.poll_shutdown()?;
        if progress.deadline_expired && !progress.settled && !self.shutdown_expiry_observed {
            self.shutdown_expiry_observed = true;
            return Err("Output child remains owned after original local shutdown budget".into());
        }
        Ok(progress.settled)
    }
    fn finish_shutdown(&mut self) -> Result<()> {
        if !self.poll_shutdown()? {
            return Err("Output child unresolved shutdown".into());
        }
        if let Some(owner) = &mut self.owner {
            owner.finish_shutdown()?;
        }
        self.channel = None;
        self.native_scope = None;
        self.scope = None;
        self.client = None;
        self.owner = None;
        self.finished = true;
        Ok(())
    }
}
