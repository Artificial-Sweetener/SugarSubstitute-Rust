// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe actual family and execution settlement before releasing installer custody.
use super::*;
use std::{num::NonZeroUsize, time::Duration};
use sugarsubstitute_contracts::execution::{
    Clock, CompletionWake, ExecutionPort, ScopeId, ScopeSpec,
};

impl Controller {
    /// Admit the current native transcript placement without waiting for preparation.
    pub(super) fn console_layout(
        &mut self,
        layout: sugarsubstitute_desktop::installer::InstallerConsoleLayout,
    ) -> Result<()> {
        self.console.as_mut().ok_or("Installer console absent")?.layout(layout)
    }
    /// Borrow the retained native transcript and its original preparation lease.
    pub(super) fn console(
        &self,
    ) -> Option<sugarsubstitute_desktop::installer::InstallerConsole<'_>> {
        self.console.as_ref()?.view(
            sugarsubstitute_contracts::localization::installer::messages(&self.locale).details,
        )
    }
    /// Start the single installer execution root and authenticated package preparation.
    pub(super) fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<()> {
        if self.owner.is_some() || self.closing {
            return Err("Installer cannot start twice".into());
        }
        self.owner = Some(ExecutionOwner::new(self.clock.clone(), wake)?);
        let client = self.owner.as_ref().ok_or("Installer execution missing")?.client();
        self.scope =
            Some(client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?);
        self.client = Some(client);
        self.console = Some(super::super::console::Output::new()?);
        let root = self.bundle_root.clone();
        let workspace = self.workspace.clone();
        let distribution = self.distribution;
        self.submit(StorageWork::Payload, move |context| {
            let distribution = super::super::distribution::decode(distribution, &context)?;
            let expected = distribution.manifest_digest().to_owned();
            drop(distribution);
            let cache = setup_storage::cache::RuntimeCache::open(
                &workspace,
                setup_storage::cache::CachePolicy::default(),
                &context,
            )?;
            let bundle = setup_storage::materialize(&root, &cache, &expected, &context)?;
            Ok(StorageResult::Bundle(Arc::new(bundle), cache))
        })
    }
    /// Close admission while retaining original cleanup obligations.
    pub(super) fn close(&mut self) -> Result<()> {
        self.closing = true;
        self.waiting = None;
        if let Some(storage) = &self.storage {
            storage.cancel_preparation();
        }
        if let Some(worker) = &mut self.worker {
            worker.close()?;
        }
        Ok(())
    }
    /// Observe actual process and execution settlement before releasing custody.
    pub(super) fn settled(&mut self) -> Result<bool> {
        if !self.closing
            || self.storage.is_some()
            || self.console.as_ref().is_some_and(|console| !console.settled())
            || self.worker.as_ref().is_some_and(|worker| !worker.settled())
        {
            return Ok(false);
        }
        if let Some(relaunch) = self.relaunch.clone() {
            self.retire_launch(relaunch)?;
            return Ok(false);
        }
        let Some(owner) = &mut self.owner else {
            return Ok(true);
        };
        if !self.stopping {
            self.worker = None;
            self.stopping = true;
            owner.request_shutdown(self.clock.now().checked_add(Duration::from_secs(15))?);
        }
        let state = owner.poll_shutdown()?;
        if state.deadline_expired && !state.settled {
            return Err("Installer execution has not settled".into());
        }
        Ok(state.settled)
    }
    /// Release already-settled installer resources after native window teardown.
    pub(super) fn finish(&mut self) -> Result<()> {
        if !self.settled()? {
            return Err("Installer still owns active work".into());
        }
        if let Some(owner) = &mut self.owner {
            owner.finish_shutdown()?;
        }
        self.prepared = None;
        self.integration = None;
        self.installed = None;
        self.relaunch = None;
        self.launch_failure = None;
        self.console = None;
        self.bundle = None;
        self.cache = None;
        self.scope = None;
        self.client = None;
        self.owner = None;
        Ok(())
    }
}

impl Controller {
    /// Observe owned work and drain a bounded batch without a presentation serialization step.
    pub(super) fn advance_events(&mut self) -> Result<Advance> {
        let mut completions = sugarsubstitute_contracts::execution::CompletionBatch {
            notifications: Vec::new(),
            remaining: 0,
        };
        if let Some(client) = &self.client {
            completions = client
                .drain_completions(NonZeroUsize::new(32).ok_or("Invalid installer drain limit")?)?;
            if let Some(worker) = &mut self.worker {
                match worker.advance(&completions) {
                    Ok(events) => {
                        for event in events {
                            if !self.closing
                                && !self.unavailable
                                && let Err(cause) = self.worker_event(event)
                            {
                                self.worker_failed(cause)?;
                            }
                        }
                    }
                    Err(cause) if !self.closing => self.worker_failed(cause)?,
                    Err(cause) => return Err(cause),
                }
            }
            self.poll_storage()?;
        }
        if let (Some(console), Some(client), Some(scope)) =
            (&mut self.console, &self.client, &self.scope)
        {
            console.advance(client, scope, &self.jobs, self.closing)?;
        }
        let waiting_ready = self.bundle.is_some()
            && self.waiting.as_ref().is_some_and(|request| {
                !request.needs_worker() || self.worker.as_ref().is_none_or(Worker::ready)
            });
        if waiting_ready && let Some(request) = self.waiting.take() {
            self.accept(request)?;
        }
        Ok(Advance {
            completions,
            events: self.output.drain(..self.output.len().min(16)).collect(),
        })
    }
}

#[cfg(test)]
mod tests;
