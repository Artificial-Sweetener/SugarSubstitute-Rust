// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare typed cache grants off the UI thread and finalize only actually retired native writers.
use super::*;
use std::io;
use sugarsubstitute_supervision::{ProvisioningEndpoint, SettledProvisioning};

/// Preserve original actually settled resources across a fallible storage completion.
pub(super) struct CacheFinish {
    pub(super) resources: Option<SettledProvisioning>,
    pub(super) failure: Option<io::Error>,
}
impl Worker {
    /// Prepare authenticated input without environment capture or filesystem work on the UI thread.
    pub(super) fn prepare_input(&mut self) -> Result<()> {
        let (port, token) = self.endpoint.take().ok_or("Provisioning endpoint absent")?;
        let bundle = self.bundle.clone();
        let cache = self.cache.clone();
        let workspace = self.workspace.clone();
        let owner = self.identity;
        let mut specification = self.spec(
            JobClass::InfrastructureLifecycle,
            16 * 1024 * 1024,
            Some(self.client.now()?.checked_add(Duration::from_secs(30))?),
        )?;
        specification.operation = Operation::Infrastructure;
        self.preparing =
            Some(self.client.submit_blocking(&self.scope, specification, move |context| {
                let environment = crate::worker_environment::capture().map_err(io::Error::other)?;
                WindowsLaunchInput::provisioning(
                    owner,
                    bundle,
                    &cache,
                    &workspace,
                    ProvisioningEndpoint { port, token },
                    environment,
                    &context,
                )
            })?);
        Ok(())
    }
    /// Consume original storage outcomes before admission or shutdown progresses.
    pub(super) fn poll_cache_jobs(&mut self) -> Result<()> {
        if let Some(job) = &mut self.preparing
            && let Some(completion) = job.try_take_completion()
        {
            self.preparing = None;
            match completion.outcome {
                JobOutcome::Success(input) if !self.closing => self.launch_input(input)?,
                JobOutcome::Success(_) | JobOutcome::Cancelled if self.closing => {}
                JobOutcome::Failure(cause) => return Err(cause.into()),
                _ => return Err("Provisioning input did not complete".into()),
            }
        }
        if let Some(job) = &mut self.cache_job
            && let Some(completion) = job.try_take_completion()
        {
            self.cache_job = None;
            match completion.outcome {
                JobOutcome::Success(result) => {
                    self.retired = result.resources;
                    if let Some(cause) = result.failure {
                        return Err(cause.into());
                    }
                    self.retired = None;
                    self.cache_finalized = true;
                }
                JobOutcome::Failure(cause) => return Err(cause.into()),
                _ => return Err("Provisioning cache finalization did not complete".into()),
            }
        }
        Ok(())
    }
    /// Schedule storage only after the native registry relinquishes all process ownership.
    pub(super) fn finalize_cache(&mut self) -> Result<()> {
        if self.cache_finalized
            || self.cache_job.is_some()
            || self.preparing.is_some()
            || self.creating.is_some()
            || self.process.is_some()
        {
            return Ok(());
        }
        let mut resources = self.retired.take();
        let pressure = self.pressure;
        let cache = self.cache.clone();
        let mut specification = self.spec(
            JobClass::InfrastructureLifecycle,
            8 * 1024 * 1024,
            Some(self.client.now()?.checked_add(Duration::from_secs(30))?),
        )?;
        specification.operation = Operation::Infrastructure;
        self.cache_job =
            Some(self.client.submit_blocking(&self.scope, specification, move |context| {
                let result = match &mut resources {
                    Some(resources) => resources.finish(pressure, &context),
                    None => sugarsubstitute_supervision::setup_storage::recover_provisioning_cache(
                        &cache, &context,
                    ),
                };
                let result = result.and_then(|()| {
                    resources = None;
                    cache.maintain(
                        sugarsubstitute_supervision::setup_storage::cache::utc_seconds()?,
                        &context,
                    )?;
                    Ok(())
                });
                Ok(CacheFinish { resources, failure: result.err() })
            })?);
        Ok(())
    }
}
