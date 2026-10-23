// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe actual family and execution settlement before releasing installer custody.
use super::*;
use std::{num::NonZeroUsize, time::Duration};
use sugarsubstitute_contracts::execution::{
    Clock, CompletionWake, ExecutionPort, ScopeId, ScopeSpec,
};
use sugarsubstitute_desktop::installer::InstallerLifetime;

impl InstallerLifetime for Controller {
    fn prepare_browser(&mut self) {
        self.profiles.request();
    }
    fn profile_path(&self) -> Option<PathBuf> {
        self.profiles.path()
    }
    fn recovering_profile(&self) -> bool {
        self.profiles.recovering()
    }
    fn browser_retired(
        &mut self,
        outcome: sugarsubstitute_desktop::installer::BrowserRetirement,
    ) -> Result<()> {
        self.profiles.retired(outcome)
    }
    fn environment_observed(&mut self, path: PathBuf) -> Result<()> {
        self.profiles.observed(path)
    }
    fn environment_ready(&self) -> bool {
        self.profiles.ready()
    }
    fn console_layout(
        &mut self,
        layout: sugarsubstitute_desktop::installer::InstallerConsoleLayout,
    ) -> Result<()> {
        self.console.as_mut().ok_or("Installer console absent")?.layout(layout)
    }
    fn console(&self) -> Option<sugarsubstitute_desktop::installer::InstallerConsole<'_>> {
        self.console.as_ref()?.view(
            sugarsubstitute_contracts::localization::installer::messages(&self.locale)["details"],
        )
    }
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<()> {
        if self.owner.is_some() || self.closing {
            return Err("Installer cannot start twice".into());
        }
        self.owner = Some(ExecutionOwner::new(self.clock.clone(), wake)?);
        let client = self.owner.as_ref().ok_or("Installer execution missing")?.client();
        self.scope =
            Some(client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?);
        self.client = Some(client);
        self.console = Some(super::super::console::Output::new()?);
        if self.profiles.requested() { Ok(()) } else { self.start_payload() }
    }

    fn request(&mut self, json: &str) -> Result<()> {
        self.request_value(json)
    }
    fn advance(&mut self) -> Result<Vec<String>> {
        if let (Some(client), Some(scope)) = (&self.client, &self.scope) {
            self.profiles.advance(client, scope, &self.jobs, &self.workspace)?;
        }
        if !self.closing
            && !self.payload_started
            && self.profiles.requested()
            && self.profiles.path().is_some()
            && !self.profiles.recovering()
        {
            self.start_payload()?;
        }
        if let Some(client) = &self.client {
            let batch = client
                .drain_completions(NonZeroUsize::new(32).ok_or("Invalid installer drain limit")?)?;
            if let Some(worker) = &mut self.worker {
                match worker.advance(&batch) {
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
                request["operation"] == "location" || self.worker.as_ref().is_none_or(Worker::ready)
            });
        if waiting_ready && let Some(request) = self.waiting.take() {
            self.request_value(&serde_json::to_string(&request)?)?;
        }
        self.output
            .drain(..self.output.len().min(16))
            .map(|value| serde_json::to_string(&value).map_err(Into::into))
            .collect()
    }
    fn close(&mut self) -> Result<()> {
        self.closing = true;
        self.waiting = None;
        if let Some(storage) = &self.storage {
            storage.cancel_preparation();
        }
        self.profiles.close();
        if let Some(worker) = &mut self.worker {
            worker.close()?;
        }
        Ok(())
    }
    fn work_settled(&mut self) -> Result<bool> {
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
        Ok(true)
    }
    fn settled(&mut self) -> Result<bool> {
        if !self.work_settled()? || !self.profiles.settled() {
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
    fn finish(&mut self) -> Result<()> {
        if !self.settled()? {
            return Err("Installer still owns active work".into());
        }
        if let Some(owner) = &mut self.owner {
            owner.finish_shutdown()?;
        }
        self.prepared = None;
        self.installed = None;
        self.relaunch = None;
        self.launch_failure = None;
        self.console = None;
        self.profiles = profiles::Profiles::default();
        self.bundle = None;
        self.scope = None;
        self.client = None;
        self.owner = None;
        Ok(())
    }
}

impl Controller {
    /// Admit source materialization after GUI profile initialization has released the cache writer.
    fn start_payload(&mut self) -> Result<()> {
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
        })?;
        self.payload_started = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
