// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep native claims/families before actual execution and headless teardown.
use super::*;
impl HeadlessLifetime for Fixture {
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<()> {
        if self.owner.is_some() || self.closing {
            return Err("Claim campaign cannot start twice".into());
        }
        self.wake = Some(wake.clone());
        self.owner = Some(ExecutionOwner::new(self.clock.clone(), wake)?);
        let client = self.owner.as_ref().ok_or("Claim execution owner missing")?.client();
        self.client = Some(client.clone());
        // Create cleanup before fallible work admission. Native ownership does
        // not exist if these native-free scope constructors fail.
        self.cleanup_scope =
            Some(client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?);
        self.scope = Some(client.create_scope(
            ScopeSpec { id: ScopeId::new(2)?, deadline: Some(self.execution_deadline) },
            None,
        )?);
        self.native_scope = Some(
            client.create_native_scope(ScopeSpec { id: ScopeId::new(3)?, deadline: None }, None)?,
        );
        self.setup_workspace()?;
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
            || self.cleanup_scope.is_none()
            || self.native_scope.is_none()
            || self.scope.is_none()
        {
            return Err("Fixture original execution/native admission is incomplete".into());
        }
        Ok(sugarsubstitute_desktop::StartupPreparationStep::Ready)
    }
    fn activity(&mut self, max: NonZeroUsize) -> Result<HeadlessStep> {
        self.advance(max)
    }
    fn next_deadline(&self) -> Option<Instant> {
        [
            self.watch.next_deadline(),
            self.managed.as_ref().and_then(|f| f.stop.as_ref()).and_then(StopPlan::next_deadline),
            self.child
                .as_ref()
                .and_then(|c| c.family.as_ref())
                .and_then(|f| f.stop.as_ref())
                .and_then(StopPlan::next_deadline),
        ]
        .into_iter()
        .flatten()
        .min()
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
        if let Some(registry) = &mut self.registry {
            registry.close_launch_admission();
        }
        if let Some(scope) = &self.scope {
            scope.cancel();
        }
        self.notify()
    }
    fn poll_shutdown(&mut self) -> Result<bool> {
        if self.registry_creation.is_some() {
            return Ok(false);
        }
        if !self.closing
            || (self.initialization_admitted && !self.claims_closed)
            || self.claims_pending
            || self.setup.is_some()
            || self.registry.as_ref().is_some_and(|r| !r.is_empty())
            || self.child.as_ref().is_some_and(|c| !c.event_retired || c.read.is_some())
        {
            return Ok(false);
        }
        let Some(owner) = &mut self.owner else { return Ok(true) };
        if !self.execution_stopping {
            self.execution_stopping = true;
            drop(self.registry.take());
            owner.request_shutdown(self.execution_deadline);
        }
        let progress = owner.poll_shutdown()?;
        if progress.deadline_expired && !self.custody_expired {
            self.custody_expired = true;
            return Err("Claim execution has not settled before its custody bound".into());
        }
        if progress.settled {
            self.workers = Some(progress.workers);
        }
        Ok(progress.settled)
    }
    fn finish_shutdown(&mut self) -> Result<()> {
        if !self.poll_shutdown()? {
            return Err("Claim fixture cannot finish retained native ownership".into());
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
