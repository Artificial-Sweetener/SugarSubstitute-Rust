// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare real profile, direct UI/control pipes and suspended carrier custody before ACK.
use super::*;
use crate::production::entry_activation::desktop_output::DesktopOutput;
use std::os::windows::ffi::OsStrExt;
use sugarsubstitute_execution::{
    windows::NativeCorrelation, windows_lifecycle_pipe::MIN_LIFECYCLE_PIPE_BYTES,
};
use sugarsubstitute_supervision::{WindowsLaunchCommand, profiles::PROFILE_OPERATION_BYTES};
impl DesktopStart {
    /// Drive the exact successor profile and all original thirty-second launch inputs.
    /// Previous profile and family retirement are independently checked by the profile owner.
    pub(in crate::production::entry_activation) fn prepare_recovery(
        &mut self,
        previous: &WindowsProfileLease,
        parent: &BackendCustodyParent,
        executable: &Path,
        working_directory: &Path,
        resources: &mut EntryResources<'_>,
    ) -> Result<bool> {
        let grant = self.grant().ok_or("Actual recovery grant missing")?.clone();
        if self.recovery_committed() {
            return Ok(true);
        }
        if self.recovery.as_ref().ok_or("Actual recovery mode missing")?.state()?.closing.is_some()
        {
            return Err("Recovery preparation already closing".into());
        }
        if !self.acquired {
            let root_watch = native_spec(resources, 4096)?;
            let native_scope = ScopeSpec { id: resources.scopes.allocate()?, deadline: None };
            let spec =
                finite_spec(resources, grant.original_budget().until(), PROFILE_OPERATION_BYTES)?;
            self.profile.submit_recovery_custodied_acquire(
                resources.client,
                resources.authority.registry_mut().ok_or("Actual recovery registry missing")?,
                resources.finite,
                spec,
                parent,
                (native_scope, root_watch),
                &grant,
                previous,
            )?;
            self.acquired = true;
            self.custody_watch = Some((native_scope, root_watch));
            return Ok(false);
        }
        self.acquire_profile(parent, resources)?;
        if !self.profile.snapshot().held
            || self.profile.snapshot().pending
            || !self.profile.custody_publication_ready()?
        {
            return Ok(false);
        }
        self.reserve_recovery_channel(resources)?;
        if self.output.is_none() {
            self.output = Some(DesktopOutput::new(
                self.run,
                resources.client,
                resources.native,
                resources.jobs,
            )?);
        }
        if !self.output.as_ref().is_some_and(DesktopOutput::prepared) {
            return Ok(false);
        }
        self.prepare_recovery_bootstrap()?;
        if self.ticket.is_none() {
            let state =
                self.recovery.as_mut().ok_or("Actual recovery mode missing")?.state_mut()?;
            if state.input.is_none() {
                let Some(paired) = state.paired.as_ref() else {
                    return Ok(false);
                };
                let token = self.profile.recovery_job_token(&grant)?;
                let arguments =
                    crate::production::recovery_supervisor::role::arguments(paired, token)
                        .map_err(crate::production::entry_activation::outer)?;
                let command = WindowsLaunchCommand::from_utf16(
                    &executable.as_os_str().encode_wide().collect::<Vec<_>>(),
                    &working_directory.as_os_str().encode_wide().collect::<Vec<_>>(),
                    &arguments,
                )?;
                let environment = match &mut self.projection.outcome {
                    JobOutcome::Success(value) => value.desktop_environment.take(),
                    _ => None,
                }
                .ok_or("Actual recovery desktop environment consumed")?;
                let paired = state.paired.take().ok_or("Actual recovery section pair missing")?;
                let enrollment =
                    state.enrollment.take().ok_or("Actual recovery carrier enrollment missing")?;
                let input = WindowsLaunchInput::with_recovery_carrier(
                    command,
                    environment,
                    paired,
                    enrollment,
                    &grant,
                )?;
                state.input = Some(
                    self.output.as_mut().ok_or("Actual recovery output missing")?.attach(input)?,
                );
            }
            let bytes = state
                .input
                .as_ref()
                .ok_or("Actual recovery input missing")?
                .required_reservation_bytes();
            let spec = finite_spec(resources, grant.original_budget().until(), bytes)?;
            self.ticket = Some(self.profile.reserve_recovery_desktop_launch(
                resources.client,
                resources.authority.registry_mut().ok_or("Actual recovery registry missing")?,
                resources.finite,
                spec,
                &grant,
                previous,
                &mut state.input,
            )?);
            self.attempted = true;
        }
        let ticket = self.ticket.ok_or("Actual held recovery ticket missing")?;
        let state = self.recovery.as_mut().ok_or("Actual recovery mode missing")?.state_mut()?;
        if state.desktop_watch.is_none() {
            let spec = native_spec(resources, 4096)?;
            state.desktop_watch = Some(
                resources
                    .authority
                    .registry_mut()
                    .ok_or("Actual recovery registry missing")?
                    .reserve_recovery_desktop_watch(resources.client, ticket, &grant, spec)?,
            );
        }
        if state.handler_permit.is_none() {
            state.handler_permit = Some(resources.client.reserve_process_watch(
                resources.native,
                native_spec(resources, 4096)?,
                NativeCorrelation::new(ticket.process().id.get())?,
            )?);
        }
        self.recovery_acknowledgement_allowed(previous, resources)
    }
    /// Reserve ordinary v1 direct Root/UI ownership and the distinct authenticated recovery control.
    pub(in crate::production::entry_activation) fn reserve_recovery_channel(
        &mut self,
        resources: &EntryResources<'_>,
    ) -> Result<()> {
        let grant = self.grant().ok_or("Actual recovery grant missing")?.clone();
        if self.pipe.is_none() {
            self.pipe = Some(resources.client.reserve_lifecycle_pipe_server(
                resources.native,
                native_spec(resources, MIN_LIFECYCLE_PIPE_BYTES)?,
                self.run,
                grant.original_budget().until(),
            )?);
        }
        let state = self.recovery.as_mut().ok_or("Actual recovery mode missing")?.state_mut()?;
        if state.pipe.is_none() {
            state.pipe = Some(resources.client.reserve_recovery_pipe_server(
                resources.native,
                native_spec(resources, MIN_LIFECYCLE_PIPE_BYTES)?,
                &grant,
            )?);
        }
        Ok(())
    }
    fn prepare_recovery_bootstrap(&mut self) -> Result<()> {
        let state = self.recovery.as_mut().ok_or("Actual recovery mode missing")?.state_mut()?;
        if state.paired.is_some() || state.input.is_some() || self.ticket.is_some() {
            return Ok(());
        }
        let direct = self.pipe.as_ref().ok_or("Actual direct UI transport missing")?;
        if self.bootstrap.is_none() {
            let Some(bootstrap) = direct.control.take_bootstrap()? else {
                return Ok(());
            };
            self.original = bootstrap.deadline();
            self.bootstrap = Some(bootstrap);
            self.enrollment = Some(direct.control.take_desktop_enrollment()?);
        }
        let recovery = state.pipe.as_mut().ok_or("Actual recovery transport missing")?;
        if state.bootstrap.is_none() {
            let Some((bootstrap, enrollment)) = recovery.take_bootstrap()? else {
                return Ok(());
            };
            state.bootstrap = Some(bootstrap);
            state.enrollment = Some(enrollment);
        }
        let root = state.bootstrap.take().ok_or("Actual recovery section missing")?;
        let desktop = self.bootstrap.take().ok_or("Actual direct UI section missing")?;
        match recovery.control().pair_desktop_bootstrap(
            &state.grant,
            &direct.control,
            root,
            desktop,
        ) {
            Ok(pair) => state.paired = Some(pair),
            Err(rejected) => {
                state.bootstrap = Some(rejected.recovery);
                self.bootstrap = Some(rejected.desktop);
                return Err(rejected.error.into());
            }
        }
        Ok(())
    }
    /// Check actual held native input/profile publication, both independent watches and original pins.
    pub(in crate::production::entry_activation) fn recovery_acknowledgement_allowed(
        &self,
        previous: &WindowsProfileLease,
        resources: &EntryResources<'_>,
    ) -> Result<bool> {
        let state = self.recovery.as_ref().ok_or("Actual recovery mode missing")?.state()?;
        if state.closing.is_some()
            || state.active.is_some()
            || state.cause.is_some()
            || state.creation_begun
            || state.input.is_some()
            || state.paired.is_some()
            || state.bootstrap.is_some()
            || state.enrollment.is_some()
            || self.enrollment.is_none()
            || self.original.is_none()
            || self.ticket.is_none()
            || state.desktop_watch.is_none()
            || state
                .session_memory
                .as_ref()
                .is_none_or(|claim| claim.bytes() != RootRecoverySession::required_storage_bytes())
            || state.handler_permit.as_ref().is_none_or(|permit| permit.deadline().is_some())
        {
            return Ok(false);
        }
        let execution = state.grant.execution_id()?;
        if state.handler_permit.as_ref().is_none_or(|permit| permit.owner_id() != execution) {
            return Ok(false);
        }
        let pipe = state.pipe.as_ref().ok_or("Actual recovery transport missing")?;
        if pipe.control().prepared_key() != Some(state.grant.key()) {
            return Ok(false);
        }
        pipe.control().validate_current_admission()?;
        self.pipe
            .as_ref()
            .ok_or("Actual direct UI transport missing")?
            .control
            .validate_current_admission()?;
        let registry = resources.authority.registry().ok_or("Actual recovery registry missing")?;
        self.profile
            .recovery_launch_acknowledgement_allowed(registry, &state.grant, previous)
            .map_err(Into::into)
    }
}
fn native_spec(resources: &EntryResources<'_>, bytes: u64) -> Result<JobSpec> {
    Ok(JobSpec {
        id: resources.jobs.allocate()?,
        operation: Operation::NativeLifecycle,
        class: JobClass::NativeObservation,
        deadline: None,
        reservation_bytes: bytes,
        replacement: None,
    })
}
fn finite_spec(resources: &EntryResources<'_>, until: Moment, bytes: u64) -> Result<JobSpec> {
    Ok(JobSpec {
        id: resources.jobs.allocate()?,
        operation: Operation::NativeLifecycle,
        class: JobClass::InfrastructureLifecycle,
        deadline: Some(until),
        reservation_bytes: bytes,
        replacement: None,
    })
}
