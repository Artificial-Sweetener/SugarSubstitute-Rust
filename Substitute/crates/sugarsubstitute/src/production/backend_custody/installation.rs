// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own helper installation-storage jobs separately from cancellation of the native custody pool.
use std::{error::Error, sync::Arc};
use sugarsubstitute_contracts::execution::{
    CompletionBatch, CompletionWake, ExecutionPort, JobId, Scope, ScopeId, ScopeSpec,
};
use sugarsubstitute_execution::ExecutionClient;
use sugarsubstitute_supervision::{BackendCustodyInstallation, BackendCustodySource};

#[derive(Default)]
pub(super) struct Storage {
    worker: Option<BackendCustodyInstallation>,
    scope: Option<Scope>,
    wake: Option<Arc<dyn CompletionWake>>,
}
impl Storage {
    /// Reserve an independent scope before the native source can request any storage work.
    pub(super) fn initialize(
        &mut self,
        client: &ExecutionClient,
        wake: Arc<dyn CompletionWake>,
    ) -> Result<(), Box<dyn Error>> {
        if self.scope.is_some() {
            return Err("Installation storage scope repeated".into());
        }
        self.scope =
            Some(client.create_scope(ScopeSpec { id: ScopeId::new(3)?, deadline: None }, None)?);
        self.wake = Some(wake);
        Ok(())
    }
    /// Native request publication wakes this same helper's existing headless event loop.
    pub(super) fn wake(&self) -> Result<Arc<dyn CompletionWake>, Box<dyn Error>> {
        self.wake.clone().ok_or_else(|| "Installation storage wake absent".into())
    }
    /// Take the source's sole job observer before its native driver starts.
    pub(super) fn bind(&mut self, source: &BackendCustodySource) -> Result<(), Box<dyn Error>> {
        if self.worker.is_some() {
            return Err("Installation storage controller repeated".into());
        }
        self.worker = Some(source.take_installation_worker()?);
        Ok(())
    }
    /// Observe exact outcomes and admit pending work without blocking the headless thread.
    /// Pipe, native custody and capture use IDs 1–3; these two one-shot operations use 4–5.
    pub(super) fn advance(
        &mut self,
        client: &ExecutionClient,
        batch: &CompletionBatch,
    ) -> Result<(), Box<dyn Error>> {
        let Some(worker) = &mut self.worker else {
            return Ok(());
        };
        worker.observe(batch)?;
        worker.advance(
            client,
            self.scope.as_ref().ok_or("Installation storage scope absent")?,
            JobId::new(4)?,
            JobId::new(5)?,
        )?;
        Ok(())
    }
}
