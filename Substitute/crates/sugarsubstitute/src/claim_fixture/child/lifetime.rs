// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retire pre-bind admission, real claims and workers before a normal child exit.
use super::*;
impl HeadlessLifetime for Child {
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<()> {
        if self.owner.is_some() || self.closing {
            return Err("Claim child cannot start twice".into());
        }
        self.wake = Some(wake.clone());
        self.owner = Some(ExecutionOwner::new(self.clock.clone(), wake)?);
        let client = self.owner.as_ref().ok_or("Claim child execution owner missing")?.client();
        self.client = Some(client.clone());
        self.cleanup_scope =
            Some(client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?);
        self.native_scope = Some(
            client.create_native_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?,
        );
        let permit = client.reserve_fixture_child(
            self.native_scope.as_ref().ok_or("Claim child native scope missing")?,
            JobSpec {
                id: JobId::new(1)?,
                operation: Operation::NativeLifecycle,
                class: JobClass::NativeObservation,
                deadline: None,
                reservation_bytes: MIN_FIXTURE_SESSION_BYTES,
                replacement: None,
            },
            self.options.correlation,
            self.options.deadline,
        )?;
        let deadline = permit.deadline();
        self.deadline = Some(deadline);
        self.permit = Some(permit);
        // Sample Instant before the local Moment. This maps the already admitted
        // budget conservatively; it does not translate or renew the QPC deadline.
        let native_now = std::time::Instant::now();
        let local_now = self.clock.now();
        self.native_deadline =
            Some(super::super::specification::native_deadline(native_now, local_now, deadline)?);
        self.scope = Some(
            client
                .create_scope(ScopeSpec { id: ScopeId::new(3)?, deadline: Some(deadline) }, None)?,
        );
        // Cleanup admission remains available after original work cancellation;
        // shutdown expiry still uses the immutable original admitted deadline.
        let spec = self.spec(CLAIM_POOL_BYTES, false)?;
        self.claims.submit_initialize(
            &client,
            self.scope.as_ref().ok_or("Claim child work scope missing")?,
            spec,
            self.options.installation.clone(),
        )?;
        self.initialization_admitted = true;
        self.pending = true;
        self.notify()
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
            || self.cleanup_scope.is_none()
            || self.native_scope.is_none()
            || self.scope.is_none()
            || !self.initialization_admitted
        {
            return Err("Fixture original execution/native admission is incomplete".into());
        }
        Ok(sugarsubstitute_desktop::StartupPreparationStep::Ready)
    }
    fn activity(&mut self, max: NonZeroUsize) -> Result<HeadlessStep> {
        self.advance(max)
    }
    fn next_deadline(&self) -> Option<std::time::Instant> {
        (!self.deadline_reported).then_some(self.native_deadline).flatten()
    }
    fn request_shutdown(&mut self) -> Result<()> {
        if self.closing {
            return Ok(());
        }
        self.closing = true;
        if let Some(pending) = &self.registry_creation {
            pending.cancel();
        }
        self.claims.close_admission();
        if let Some(scope) = &self.scope {
            scope.cancel();
        }
        if let Some(permit) = self.permit.take() {
            self.protocol = Some(permit.cancel_unbound());
        }
        if let Some(job) = &self.protocol {
            job.cancel();
        }
        self.notify()
    }
    fn poll_shutdown(&mut self) -> Result<bool> {
        if self.registry_creation.is_some() {
            return Ok(false);
        }
        if !self.closing
            || (self.initialization_admitted && !self.claims_closed)
            || self.pending
            || self.protocol.is_some()
            || self.permit.is_some()
        {
            return Ok(false);
        }
        let Some(owner) = &mut self.owner else { return Ok(true) };
        if !self.execution_stopping {
            self.execution_stopping = true;
            drop(self.registry.take());
            // If admission itself failed, no translated budget or native job
            // exists. Immediate shutdown does not create a replacement deadline.
            let deadline = self.deadline.unwrap_or_else(|| self.clock.now());
            owner.request_shutdown(deadline);
        }
        let progress = owner.poll_shutdown()?;
        if progress.deadline_expired && !self.deadline_reported {
            self.deadline_reported = true;
            return Err("Claim child cleanup exceeded its original admitted deadline".into());
        }
        if progress.settled {
            self.workers = Some(progress.workers);
        }
        Ok(progress.settled)
    }
    fn finish_shutdown(&mut self) -> Result<()> {
        if !self.poll_shutdown()? {
            return Err("Claim child cannot finish unresolved native state".into());
        }
        if let Some(owner) = &mut self.owner {
            owner.finish_shutdown()?;
        }
        self.client = None;
        self.owner = None;
        self.wake = None;
        self.finished = true;
        Ok(())
    }
}
