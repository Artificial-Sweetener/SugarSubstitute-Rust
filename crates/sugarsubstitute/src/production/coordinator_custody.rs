// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose one actual independent headless witness for managed backends.
mod attachment;
use super::coordinator_backend::{BackendResources, Result};
use std::{
    num::NonZeroUsize,
    os::windows::{ffi::OsStrExt, io::AsRawHandle},
    path::PathBuf,
};
use sugarsubstitute_contracts::lifecycle::{ResourceKey, ShutdownBudget, ShutdownId};
use sugarsubstitute_contracts::{
    backend_custody::{BackendCustodyKey, BackendCustodySession},
    execution::{JobClass, JobSpec, Moment, Operation, ScopeSpec},
    lifecycle::CoordinatorKey,
};
use sugarsubstitute_supervision::{
    BackendCustodyParent, LaunchEnvironment, LaunchTicket, WindowsLaunchCommand, WindowsLaunchInput,
};
/// Retain the actual controller and immutable invocation through checked retirement.
pub(crate) struct CoordinatorCustody {
    parent: Option<BackendCustodyParent>,
    attachment: attachment::Attachment,
    executable: PathBuf,
    installation: PathBuf,
    environment: Option<LaunchEnvironment>,
    ticket: Option<LaunchTicket>,
    until: Moment,
    establishment_issued: bool,
    ready: bool,
    shutdown: Option<ShutdownBudget>,
    finish_received: bool,
    pipe_closed: bool,
    settled: bool,
    grace_observed: bool,
    total_observed: bool,
    observed_progress: Option<(bool, bool, bool, bool, bool)>,
}
impl CoordinatorCustody {
    /// Admit the original registered capture attachment before its sole transport or helper.
    pub(crate) fn new(
        owner: CoordinatorKey,
        executable: PathBuf,
        installation: PathBuf,
        environment: LaunchEnvironment,
        until: Moment,
        resources: &mut BackendResources<'_>,
    ) -> Result<Self> {
        let key = BackendCustodyKey::new(owner, BackendCustodySession::new(1)?);
        let attachment = attachment::Attachment::new(key, until, resources)?;
        Ok(Self {
            parent: None,
            attachment,
            executable,
            installation,
            environment: Some(environment),
            ticket: None,
            until,
            establishment_issued: false,
            ready: false,
            shutdown: None,
            finish_received: false,
            pipe_closed: false,
            settled: false,
            grace_observed: false,
            total_observed: false,
            observed_progress: None,
        })
    }
    /// Advance actual bootstrap, contained helper, authenticated establishment and claims binding.
    pub(crate) fn advance(&mut self, resources: &mut BackendResources<'_>) -> Result<bool> {
        if self.shutdown.is_some() {
            return Ok(false);
        }
        if self.parent.is_none() {
            self.parent = self.attachment.advance(resources)?;
        }
        let Some(parent) = self.parent.clone() else { return Ok(false) };
        if let Some(ticket) = self.ticket {
            resources
                .authority
                .registry()
                .ok_or("Custody registry absent")?
                .backend_custody_launch_pending(ticket.process(), parent.key())?;
        }
        if self.ready {
            parent.validate_current_admission()?;
            return Ok(true);
        }
        parent.consume(NonZeroUsize::new(32).ok_or("Custody observation bound missing")?)?;
        if self.ticket.is_none() && parent.prepared() {
            let (bootstrap, enrollment) =
                parent.take_bootstrap()?.ok_or("Custody bootstrap already transferred")?;
            let key = parent.key();
            let deadline = bootstrap.deadline().ok_or("Custody original native cutoff absent")?;
            let section = bootstrap.borrow_inheritance().section.as_raw_handle() as usize;
            let fields = [
                "--internal-backend-custody".to_owned(),
                "--bootstrap".into(),
                section.to_string(),
                "--until-qpc".into(),
                deadline.ticks().to_string(),
                "--qpc-frequency".into(),
                deadline.frequency().to_string(),
                "--namespace".into(),
                hex(key.owner().namespace.bytes()),
                "--epoch".into(),
                hex(key.owner().epoch.bytes()),
                "--session".into(),
                key.session().get().to_string(),
                "--installation".into(),
            ];
            let mut arguments: Vec<Vec<u16>> =
                fields.iter().map(|value| value.encode_utf16().collect()).collect();
            arguments.push(self.installation.as_os_str().encode_wide().collect());
            let command = WindowsLaunchCommand::from_utf16(
                &self.executable.as_os_str().encode_wide().collect::<Vec<_>>(),
                &self.installation.as_os_str().encode_wide().collect::<Vec<_>>(),
                &arguments,
            )?;
            let mut input = WindowsLaunchInput::new(
                command,
                self.environment.take().ok_or("Custody environment already transferred")?,
            )?;
            let mut bootstrap = Some(bootstrap);
            let mut enrollment = Some(enrollment);
            input.with_backend_custody_capture(&mut bootstrap, &mut enrollment)?;
            let root = spec(resources, JobClass::NativeObservation, 4096, None)?;
            let creation = spec(
                resources,
                JobClass::InfrastructureLifecycle,
                input.required_reservation_bytes(),
                Some(self.until),
            )?;
            let scope = ScopeSpec { id: resources.scopes.allocate()?, deadline: None };
            self.ticket = Some(
                resources
                    .authority
                    .registry_mut()
                    .ok_or("Custody process registry absent")?
                    .submit_backend_custody_launch(
                        resources.client,
                        resources.cleanup,
                        scope,
                        creation,
                        root,
                        key,
                        &mut Some(input),
                    )?,
            );
        }
        if let Some(ticket) = self.ticket {
            let started = resources
                .authority
                .registry()
                .ok_or("Custody registry absent")?
                .backend_custody_launch_pending(ticket.process(), parent.key())?;
            if started && parent.authenticated() && !self.establishment_issued {
                let job =
                    spec(resources, JobClass::InfrastructureLifecycle, 131_072, Some(self.until))?;
                resources
                    .authority
                    .registry_mut()
                    .ok_or("Custody registry absent")?
                    .submit_backend_custody_establishment(
                        resources.client,
                        resources.cleanup,
                        job,
                        &parent,
                        ticket,
                    )?;
                self.establishment_issued = true;
            }
            if self.establishment_issued
                && resources
                    .authority
                    .registry_mut()
                    .ok_or("Custody registry absent")?
                    .advance_backend_custody_establishment(&parent, ticket)?
            {
                resources.authority.claims_mut().bind_backend_custodian(
                    resources.client,
                    resources.cleanup,
                    &parent,
                )?;
                self.ready = true;
                tracing::info!(
                    operation = "backend_custody_established",
                    process_id = ticket.process().id.get(),
                    "Independent authenticated backend custody established"
                );
            }
        }
        let started = self
            .ticket
            .map(|ticket| {
                resources
                    .authority
                    .registry()
                    .ok_or("Custody registry absent")?
                    .backend_custody_launch_pending(ticket.process(), parent.key())
                    .map_err(|cause| Box::new(cause) as Box<dyn std::error::Error + Send + Sync>)
            })
            .transpose()?
            .unwrap_or(false);
        let progress = (
            parent.prepared(),
            self.ticket.is_some(),
            started,
            parent.authenticated(),
            self.establishment_issued,
        );
        if self.observed_progress != Some(progress) {
            self.observed_progress = Some(progress);
            tracing::info!(operation="backend_custody_progress", key=?parent.key(), prepared=progress.0,
                launch_submitted=progress.1, started=progress.2, authenticated=progress.3,
                establishment_issued=progress.4, "Original custody preparation phase observed");
        }
        Ok(self.ready)
    }
    /// Borrow the authentic controller for the registry's same-source release association.
    /// # Errors
    /// Rejects access before the original registered attachment admits its captured parent.
    pub(crate) fn parent(&self) -> Result<&BackendCustodyParent> {
        self.parent.as_ref().ok_or_else(|| "Actual captured custody parent absent".into())
    }
    /// Observe checked same-generation release under the backend's original cleanup.
    pub(crate) fn release(
        &self,
        resource: ResourceKey,
        budget: ShutdownBudget,
        resources: &BackendResources<'_>,
    ) -> Result<bool> {
        if self.settled {
            return Ok(true);
        }
        Ok(self.parent()?.advance_resource_release(
            resources.client,
            resource,
            budget.settle_until,
        )?)
    }
    /// Require genuine helper, native family and pipe retirement before activation closes.
    pub(crate) fn settled(&self) -> bool {
        self.settled
    }
    /// Preserve the one original witness shutdown wake.
    pub(crate) fn next_deadline(&self) -> Option<Moment> {
        self.shutdown.filter(|_| !self.settled && !self.total_observed).map(|budget| {
            if self.grace_observed { budget.settle_until } else { budget.graceful_until }
        })
    }
    /// Finish the authenticated witness, observe its real exit, then close the actual pipe.
    pub(crate) fn close(
        &mut self,
        budget: ShutdownBudget,
        resources: &mut BackendResources<'_>,
    ) -> Result<()> {
        let original = *self.shutdown.get_or_insert(budget);
        if original != budget {
            return Err("Custody original cleanup changed".into());
        }
        if self.settled {
            return Ok(());
        }
        let Some(parent) = self.parent.clone() else {
            self.settled = self.attachment.close(resources)?;
            return Ok(());
        };
        let now = resources.client.now()?;
        self.grace_observed |= now >= original.graceful_until;
        self.total_observed |= now >= original.settle_until;
        let mut failure: Option<Box<dyn std::error::Error + Send + Sync>> = None;
        if self.ready && !self.finish_received && resources.client.now()? < original.settle_until {
            match parent.advance_finish(resources.client, original.settle_until) {
                Ok(finished) => {
                    self.finish_received = finished;
                    if finished {
                        tracing::info!(
                            operation = "backend_custody_finished",
                            key = ?parent.key(),
                            settle_until = ?original.settle_until,
                            "Authenticated backend custody Finished receipt consumed"
                        );
                    }
                }
                Err(cause) => failure = Some(Box::new(cause)),
            }
        }
        if let Some(ticket) = self.ticket {
            let registry = resources.authority.registry_mut().ok_or("Custody registry absent")?;
            registry.begin_stop(ticket.process(), ShutdownId::new(1)?, original)?;
            let actual = registry.snapshot(ticket.process())?;
            if (actual.cleanup_step_due(resources.client.now()?)
                || (actual.settled && registry.custody_retirement_pending(ticket.process())?))
                && !actual.cleanup_step_pending
                && resources.client.now()? < original.settle_until
            {
                let watch = if actual.member_watch_required {
                    Some(spec(resources, JobClass::NativeObservation, 4096, None)?)
                } else {
                    None
                };
                let cleanup = spec(
                    resources,
                    JobClass::InfrastructureLifecycle,
                    4096,
                    Some(original.settle_until),
                )?;
                resources
                    .authority
                    .registry_mut()
                    .ok_or("Custody registry absent")?
                    .submit_cleanup_step(
                        resources.client,
                        ticket.process(),
                        resources.cleanup,
                        cleanup,
                        watch,
                    )?;
            }
            let registry = resources.authority.registry_mut().ok_or("Custody registry absent")?;
            if registry.snapshot(ticket.process())?.settled
                && !registry.custody_retirement_pending(ticket.process())?
            {
                while registry.take_observation(ticket.process())?.is_some() {}
                registry.retire_settled(ticket.process())?;
                self.ticket = None;
            }
        }
        if self.ticket.is_none() {
            if !self.pipe_closed {
                parent.close()?;
                self.pipe_closed = true;
            }
            if parent.take_terminal()?.is_some() {
                self.attachment.parent_retired();
                self.settled = true;
                tracing::info!(
                    operation = "backend_custody_retired",
                    key = ?parent.key(),
                    finish_received = self.finish_received,
                    "Backend custody family and transport retired"
                );
            }
        }
        match failure {
            Some(cause) => Err(cause),
            None => Ok(()),
        }
    }
}
/// Preserve class quotas and allocate IDs in actual admission order.
fn spec(
    resources: &BackendResources<'_>,
    class: JobClass,
    bytes: u64,
    deadline: Option<Moment>,
) -> Result<JobSpec> {
    Ok(JobSpec {
        id: resources.jobs.allocate()?,
        operation: Operation::NativeLifecycle,
        class,
        deadline,
        reservation_bytes: bytes,
        replacement: None,
    })
}
/// Format fixed public matching identity bytes, never capability bytes.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
