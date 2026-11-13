// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep native pump alive until every pipe and actual family retires before workers.
use super::{Result, parent::Parent};
use std::{num::NonZeroUsize, sync::Arc, time::Instant};
use sugarsubstitute_contracts::execution::{CompletionWake, ExecutionPort, ScopeId, ScopeSpec};
use sugarsubstitute_desktop::{HeadlessLifetime, HeadlessStep};
use sugarsubstitute_execution::ExecutionOwner;

impl HeadlessLifetime for Parent {
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<()> {
        if self.owner.is_some() || self.closing {
            return Err("Lifecycle parent cannot start twice/after closure".into());
        }
        self.owner = Some(ExecutionOwner::new(self.clock.clone(), wake)?);
        let client = self.owner.as_ref().ok_or("Lifecycle parent execution missing")?.client();
        self.client = Some(client.clone());
        self.scope =
            Some(client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?);
        self.pipe_scope = Some(
            client.create_native_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?,
        );
        let mut table_spec = self.spec(
            sugarsubstitute_contracts::execution::JobClass::InfrastructureLifecycle,
            sugarsubstitute_supervision::WindowsProcessRegistry::required_reservation_bytes(),
        )?;
        table_spec.deadline = Some(self.execution_deadline);
        self.registry_creation = Some(crate::process_fixture::registry::PendingRegistry::submit(
            self.client.as_ref().ok_or("Registry execution missing")?,
            self.scope.as_ref().ok_or("Registry constructor scope missing")?,
            table_spec,
            self.cases[0].run.owner,
        )?);

        self.reserve_next()
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
            || self.pipe_scope.is_none()
        {
            return Err("Fixture original execution/native admission is incomplete".into());
        }
        self.activity(max)?;
        if self.registry_creation.is_some() || self.registry.is_none() {
            return Ok(sugarsubstitute_desktop::StartupPreparationStep::Waiting);
        }
        Ok(sugarsubstitute_desktop::StartupPreparationStep::Ready)
    }
    fn activity(&mut self, max: NonZeroUsize) -> Result<HeadlessStep> {
        Parent::activity(self, max)
    }
    fn next_deadline(&self) -> Option<Instant> {
        let stall = self.watch.next_deadline();
        let stop = self
            .cases
            .get(self.current)
            .and_then(|case| case.stop.as_ref())
            .and_then(|stop| stop.next_deadline());
        match (stall, stop) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }
    fn request_shutdown(&mut self) -> Result<()> {
        if self.closing {
            return Ok(());
        }
        self.closing = true;
        if let Some(pending) = &self.registry_creation {
            pending.cancel();
        }
        if let Some(registry) = &mut self.registry {
            registry.close_launch_admission();
        }
        for case in &mut self.cases {
            if let Some(session) = &case.session {
                case.pipe_close_requested = true;
                session.control.close();
            }
        }
        self.ensure_stop()
    }
    fn poll_shutdown(&mut self) -> Result<bool> {
        if self.registry_creation.is_some() {
            return Ok(false);
        }
        if !self.closing
            || !self.registry.as_ref().is_none_or(|registry| registry.is_empty())
            || self.cases.iter().any(|case| case.session.is_some() && !case.pipe_terminal)
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
            return Err("Lifecycle execution remains owned after its custody bound".into());
        }
        if progress.settled {
            self.workers = Some(progress.workers);
        }
        Ok(progress.settled)
    }
    fn finish_shutdown(&mut self) -> Result<()> {
        if !self.poll_shutdown()? {
            return Err("Lifecycle parent cannot finish with owned native resources".into());
        }
        if let Some(owner) = &mut self.owner {
            owner.finish_shutdown()?;
        }
        for case in &mut self.cases {
            case.session = None;
        }
        self.pipe_scope = None;
        self.scope = None;
        self.client = None;
        self.owner = None;
        self.finished = true;
        Ok(())
    }
}
