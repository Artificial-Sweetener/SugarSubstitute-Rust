// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain families through actual retirement, then observe the execution owner's shutdown.
use super::*;
use sugarsubstitute_contracts::execution::CompletionWake;
use sugarsubstitute_desktop::{HeadlessLifetime, HeadlessStep};

impl HeadlessLifetime for Fixture {
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<()> {
        if self.owner.is_some() || self.closing {
            return Err("Registry fixture cannot start twice or after closure".into());
        }
        self.owner = Some(ExecutionOwner::new(self.clock.clone(), wake)?);
        let client = self.owner.as_ref().ok_or("Fixture execution owner missing")?.client();
        self.client = Some(client.clone());
        self.scope =
            Some(client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?);
        let mut table_spec = self.spec(
            sugarsubstitute_contracts::execution::JobClass::InfrastructureLifecycle,
            sugarsubstitute_supervision::WindowsProcessRegistry::required_reservation_bytes(),
        )?;
        table_spec.deadline = Some(self.execution_deadline);
        self.registry_creation = Some(crate::process_fixture::registry::PendingRegistry::submit(
            self.client.as_ref().ok_or("Registry execution missing")?,
            self.scope.as_ref().ok_or("Registry constructor scope missing")?,
            table_spec,
            self.owner_key,
        )?);

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
        if self.owner.is_none() || self.client.is_none() || self.scope.is_none() {
            return Err("Fixture original execution/native admission is incomplete".into());
        }
        self.advance(max)?;
        if self.registry_creation.is_some() || self.registry.is_none() {
            return Ok(sugarsubstitute_desktop::StartupPreparationStep::Waiting);
        }
        Ok(sugarsubstitute_desktop::StartupPreparationStep::Ready)
    }
    fn activity(&mut self, max: NonZeroUsize) -> Result<HeadlessStep> {
        self.advance(max)
    }
    fn next_deadline(&self) -> Option<Instant> {
        match (self.watch.next_deadline(), self.stop.as_ref().and_then(StopPlan::next_deadline)) {
            (Some(stall), Some(stop)) => Some(stall.min(stop)),
            (stall, stop) => stall.or(stop),
        }
    }
    fn request_shutdown(&mut self) -> Result<()> {
        self.closing = true;
        if let Some(pending) = &self.registry_creation {
            pending.cancel();
        }
        if let Some(registry) = &mut self.registry {
            registry.close_launch_admission();
        }
        self.ensure_stop()
    }
    fn poll_shutdown(&mut self) -> Result<bool> {
        if self.registry_creation.is_some() {
            return Ok(false);
        }
        if !self.closing || !self.registry.as_ref().is_none_or(|registry| registry.is_empty()) {
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
            return Err(
                "Registry fixture execution has not settled before its custody bound".into()
            );
        }
        if progress.settled {
            self.workers = Some(progress.workers);
        }
        Ok(progress.settled)
    }
    fn finish_shutdown(&mut self) -> Result<()> {
        if !self.registry.as_ref().is_none_or(|registry| registry.is_empty())
            || !self.poll_shutdown()?
        {
            return Err("Registry fixture cannot finish with retained native families/work".into());
        }
        if let Some(owner) = &mut self.owner {
            owner.finish_shutdown()?;
        }
        self.scope = None;
        self.client = None;
        self.owner = None;
        self.finished = true;
        Ok(())
    }
}
