// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep child admission, cancellation and actual three-worker shutdown on their owners.
use super::{Result, child::Child};
use std::{num::NonZeroUsize, sync::Arc, time::Instant};
use sugarsubstitute_contracts::execution::{
    Clock, CompletionWake, JobClass, JobId, JobSpec, Operation, ScopeId, ScopeSpec,
};
use sugarsubstitute_desktop::{HeadlessLifetime, HeadlessStep};
use sugarsubstitute_execution::ExecutionOwner;
use sugarsubstitute_execution::windows_lifecycle_pipe::MIN_LIFECYCLE_PIPE_BYTES;

impl HeadlessLifetime for Child {
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<()> {
        if self.owner.is_some() || self.shutdown_requested {
            return Err("Lifecycle child cannot start twice/after close".into());
        }
        self.owner = Some(ExecutionOwner::new(self.clock.clone(), wake)?);
        let client = self.owner.as_ref().ok_or("Lifecycle child owner missing")?.client();
        self.client = Some(client.clone());
        self.scope = Some(
            client.create_native_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?,
        );
        let permit = client.reserve_lifecycle_pipe_client(
            self.scope.as_ref().ok_or("Lifecycle child scope missing")?,
            JobSpec {
                id: JobId::new(1)?,
                operation: Operation::NativeLifecycle,
                class: JobClass::NativeObservation,
                deadline: None,
                reservation_bytes: MIN_LIFECYCLE_PIPE_BYTES,
                replacement: None,
            },
            self.options.deadline,
        )?;
        self.deadline = Some(permit.original_deadline());
        let section =
            self.options.section.take().ok_or("Lifecycle section token already transferred")?;
        self.session = permit.bind(section);
        if self.session.is_none() {
            return Err("Lifecycle child input reservation already consumed".into());
        }
        Ok(())
    }
    fn prepare_native_runtime(
        &mut self,
        max: NonZeroUsize,
    ) -> Result<sugarsubstitute_desktop::StartupPreparationStep> {
        if max.get() > 32 {
            return Err("Fixture preparation exceeds native drain bound".into());
        }
        if self.shutdown_requested {
            return Ok(sugarsubstitute_desktop::StartupPreparationStep::Closing);
        }
        if self.owner.is_none()
            || self.client.is_none()
            || self.scope.is_none()
            || self.session.is_none()
            || self.deadline.is_none()
        {
            return Err("Fixture original execution/native admission is incomplete".into());
        }
        Ok(sugarsubstitute_desktop::StartupPreparationStep::Ready)
    }
    fn activity(&mut self, max: NonZeroUsize) -> Result<HeadlessStep> {
        Child::activity(self, max)?;
        Ok(if self.consumed || self.shutdown_requested {
            HeadlessStep::Shutdown
        } else {
            HeadlessStep::Active
        })
    }
    fn next_deadline(&self) -> Option<Instant> {
        None
    }
    fn request_shutdown(&mut self) -> Result<()> {
        if self.shutdown_requested {
            return Ok(());
        }
        self.shutdown_requested = true;
        if let Some(session) = &self.session {
            session.control.close();
        }
        if let Some(scope) = &self.scope {
            scope.cancel();
        }
        if let Some(owner) = &mut self.owner {
            owner.request_shutdown(self.deadline.unwrap_or_else(|| self.clock.now()));
        }
        Ok(())
    }
    fn poll_shutdown(&mut self) -> Result<bool> {
        if !self.shutdown_requested {
            return Ok(false);
        }
        let Some(owner) = &self.owner else { return Ok(true) };
        let progress = owner.poll_shutdown()?;
        if progress.deadline_expired && !self.deadline_error {
            self.deadline_error = true;
            return Err(
                "Lifecycle child still owns work after its original translated deadline".into()
            );
        }
        Ok(progress.settled && (self.session.is_none() || self.consumed))
    }
    fn finish_shutdown(&mut self) -> Result<()> {
        if !self.poll_shutdown()? {
            return Err("Lifecycle child cannot finish with unresolved transport/workers".into());
        }
        if let Some(owner) = &mut self.owner {
            owner.finish_shutdown()?;
        }
        self.session = None;
        self.scope = None;
        self.client = None;
        self.owner = None;
        self.finished = true;
        Ok(())
    }
}
