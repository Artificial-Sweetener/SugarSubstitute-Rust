// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep the native pump alive through actual family, event and execution settlement.
use super::*;

impl HeadlessLifetime for Fixture {
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<()> {
        if self.owner.is_some() || self.closing {
            return Err("Held campaign cannot start twice".into());
        }
        self.wake = Some(wake.clone());
        let external = Arc::new(ExternalWake::new(wake));
        self.external_wake = Some(external.clone());
        self.owner = Some(ExecutionOwner::new(self.clock.clone(), external)?);
        let client = self.owner.as_ref().ok_or("Campaign owner missing")?.client();
        self.client = Some(client.clone());
        self.creation_scope =
            Some(client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?);
        self.event_scope = Some(
            client.create_native_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?,
        );
        let mut table_spec = self.spec(
            sugarsubstitute_contracts::execution::JobClass::InfrastructureLifecycle,
            sugarsubstitute_supervision::WindowsProcessRegistry::required_reservation_bytes(),
        )?;
        table_spec.deadline = Some(self.execution_deadline);
        self.registry_creation = Some(crate::process_fixture::registry::PendingRegistry::submit(
            self.client.as_ref().ok_or("Registry execution missing")?,
            self.creation_scope.as_ref().ok_or("Registry constructor scope missing")?,
            table_spec,
            self.owner_key,
        )?);

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
            || self.creation_scope.is_none()
            || self.event_scope.is_none()
            || self.external_wake.is_none()
        {
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
        let original = (!self.deadline_observed).then_some(self.deadline);
        self.cleanup
            .iter()
            .filter_map(|cleanup| cleanup.stop.as_ref().and_then(StopPlan::next_deadline))
            .chain(original)
            .min()
    }
    fn request_shutdown(&mut self) -> Result<()> {
        self.closing = true;
        if let Some(pending) = &self.registry_creation {
            pending.cancel();
        }
        if let Some(registry) = &mut self.registry {
            registry.close_launch_admission();
        }
        self.scan.request();
        self.notify()
    }
    fn poll_shutdown(&mut self) -> Result<bool> {
        if self.registry_creation.is_some() {
            return Ok(false);
        }
        if !self.closing
            || !self.registry.as_ref().is_none_or(|registry| registry.is_empty())
            || self.cases.iter().any(|case| case.session.is_some())
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
        if progress.deadline_expired && !self.deadline_observed {
            self.deadline_observed = true;
            return Err("Held campaign execution exceeded its original admitted deadline".into());
        }
        if progress.settled {
            self.workers = Some(progress.workers);
        }
        Ok(progress.settled)
    }
    fn finish_shutdown(&mut self) -> Result<()> {
        if !self.poll_shutdown()? {
            return Err("Held campaign cannot finish unresolved work".into());
        }
        if let Some(owner) = &mut self.owner {
            owner.finish_shutdown()?;
        }
        self.creation_scope = None;
        self.event_scope = None;
        self.client = None;
        self.owner = None;
        self.wake = None;
        self.external_wake = None;
        self.finished = true;
        Ok(())
    }
}
