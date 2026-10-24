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
    fn environment_observed(&mut self, path: PathBuf) -> Result<()> {
        if self.profile_job.is_some() || self.profile.is_some() || self.closing {
            return Err("Installer environment already observed or closing".into());
        }
        let expected = self.workspace.join("webview-v1").join(std::process::id().to_string());
        let client = self.client.as_ref().ok_or("Installer execution missing")?;
        self.profile_job = Some(client.submit_blocking(
            self.scope.as_ref().ok_or("Installer scope missing")?,
            JobSpec {
                id: self.jobs.allocate()?,
                operation: Operation::Infrastructure,
                class: JobClass::InfrastructureLifecycle,
                deadline: Some(client.now()?.checked_add(Duration::from_secs(30))?),
                reservation_bytes: 1024 * 1024,
                replacement: None,
            },
            move |context| setup_storage::InstallerProfile::acquire(&expected, &path, &context),
        )?);
        Ok(())
    }
    fn environment_ready(&self) -> bool {
        self.profile.is_some() && !self.closing
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
        let root = self.bundle_root.clone();
        let workspace = self.workspace.clone();
        let distribution = self.distribution;
        self.submit(StorageWork::Payload, move |context| {
            let distribution = super::super::distribution::decode(distribution, &context)?;
            let expected = distribution.manifest_digest().to_owned();
            drop(distribution);
            let root = setup_storage::materialize(&root, &workspace, &expected, &context)?;
            VerifiedBundle::open(&root, &expected, &context)
                .map(|bundle| StorageResult::Bundle(Arc::new(bundle)))
        })
    }
    fn request(&mut self, json: &str) -> Result<()> {
        self.request_value(json)
    }
    fn advance(&mut self) -> Result<Vec<String>> {
        if let Some(job) = &mut self.profile_job
            && let Some(completion) = job.try_take_completion()
        {
            self.profile_job = None;
            match completion.outcome {
                JobOutcome::Success(profile) => self.profile = Some(profile),
                JobOutcome::Cancelled if self.closing => {}
                JobOutcome::Failure(cause) => return Err(cause.into()),
                _ => return Err("Installer profile verification did not complete".into()),
            }
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
        if let Some(job) = &self.profile_job {
            job.cancel();
        }
        if let Some(worker) = &mut self.worker {
            worker.close()?;
        }
        Ok(())
    }
    fn settled(&mut self) -> Result<bool> {
        if !self.closing
            || self.storage.is_some()
            || self.profile_job.is_some()
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
        self.profile = None;
        self.bundle = None;
        self.scope = None;
        self.client = None;
        self.owner = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
