// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose one actual independent headless witness for managed backends.
mod attachment;
pub(super) mod diagnostics;
mod invocation;
/// Original fixed witness invocation/control metadata role; no new reservation.
pub(crate) const WITNESS_STORAGE_BYTES: u64 = 1_048_576;
mod retirement;
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
    retirement: retirement::Retirement,
    observed_progress: Option<(bool, bool, bool, bool, bool)>,
    observed_exit: Option<u32>,
}
impl CoordinatorCustody {
    /// An observed native helper exit permits independent retirement of its dead family.
    pub(crate) fn exited(&self) -> bool {
        self.observed_exit.is_some()
    }
    /// Borrow the genuine established parent for early profile Job preparation.
    pub(crate) fn established_parent(&self) -> Option<BackendCustodyParent> {
        self.ready.then(|| self.parent.clone()).flatten()
    }
    /// Admit the original registered capture attachment before its sole transport or helper.
    pub(crate) fn new(
        owner: CoordinatorKey,
        executable: PathBuf,
        installation: PathBuf,
        environment: LaunchEnvironment,
        until: Moment,
        resources: &mut BackendResources<'_>,
    ) -> Result<Self> {
        Self::validate_invocation_storage(&executable, &installation, Some(&environment))?;
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
            retirement: retirement::Retirement::default(),
            observed_progress: None,
            observed_exit: None,
        })
    }
    /// Advance actual bootstrap, contained helper, authenticated establishment and claims binding.
    pub(crate) fn advance(&mut self, resources: &mut BackendResources<'_>) -> Result<bool> {
        if self.retirement.original.is_some() {
            return Ok(false);
        }
        if self.parent.is_none() {
            self.parent = self.attachment.advance(resources)?;
        }
        let Some(parent) = self.parent.clone() else {
            return Ok(false);
        };
        let terminal = self.retirement.observe_terminal(&parent);
        // Cached original process facts remain observable after transport failure.
        let launch = (|| -> Result<()> {
            if let Some(ticket) = self.ticket {
                let registry = resources.authority.registry().ok_or("Custody registry absent")?;
                if let Some(status) = registry.snapshot(ticket.process())?.root_exit
                    && self.observed_exit != Some(status)
                {
                    self.observed_exit = Some(status);
                    let diagnostic =
                        sugarsubstitute_supervision::BackendCustodyExitDiagnostic::from_exit_code(
                            status,
                        );
                    tracing::warn!(operation="backend_custody_root_exited",key=?parent.key(),process=?ticket.process(),status,
                    first_operation=diagnostic.map(sugarsubstitute_supervision::BackendCustodyExitDiagnostic::operation),
                    "Actual authenticated helper root exited; original custody and causes remain retained");
                }
                registry.backend_custody_launch_pending(ticket.process(), parent.key())?;
            }
            Ok(())
        })();
        match (terminal, launch) {
            (Err(first), Err(later)) => {
                return Err(diagnostics::preserve_cleanup(Some(first), later));
            }
            (Err(cause), Ok(())) | (Ok(()), Err(cause)) => return Err(cause),
            (Ok(()), Ok(())) => {}
        }
        if self.ready {
            parent.maintain_liveness(resources.client)?;
            parent
                .validate_current_admission()
                .map_err(|cause| diagnostics::failed("ready_current_admission", cause))?;
            return Ok(true);
        }
        parent.consume(NonZeroUsize::new(32).ok_or("Custody observation bound missing")?)?;
        if self.ticket.is_none() && parent.prepared() {
            let (bootstrap, enrollment) =
                parent.take_bootstrap()?.ok_or("Custody bootstrap already transferred")?;
            let key = parent.key();
            let deadline = bootstrap.deadline().ok_or("Custody original native cutoff absent")?;
            let section = bootstrap.borrow_inheritance().section.as_raw_handle() as usize;
            let fields: [String; invocation::FIELDS] = [
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
                parent.maintain_liveness(resources.client)?;
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
        if self.retirement.settled {
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
        self.retirement.settled
    }
    /// Enumerate the unchanged original witness grace and total boundaries.
    pub(crate) fn deadlines(&self) -> impl Iterator<Item = Moment> + '_ {
        self.retirement.deadlines().into_iter().flatten().chain(
            self.retirement
                .original
                .is_none()
                .then(|| self.parent.as_ref().and_then(BackendCustodyParent::heartbeat_deadline))
                .flatten(),
        )
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
