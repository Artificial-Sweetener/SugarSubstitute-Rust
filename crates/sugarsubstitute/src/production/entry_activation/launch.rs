// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Transfer actual capture and profile custody into one contained desktop launch.
use super::{
    desktop_role,
    projection::{ProjectedEntry, ProjectionFailure},
};
use crate::production::entry_runner::EntryResources;
use std::{error::Error, os::windows::ffi::OsStrExt, path::Path};
use sugarsubstitute_contracts::{
    execution::{Generation, JobClass, JobCompletion, JobOutcome, JobSpec, Operation, ScopeSpec},
    lifecycle::{ConnectionId, RunKey},
    startup::LaunchId,
};
use sugarsubstitute_execution::{
    windows_capture_attach::CaptureAttachLaunchPin,
    windows_lifecycle_pipe::{
        CapturedLifecycleStart, LifecycleDesktopEnrollment, LifecyclePipeBootstrap,
        LifecyclePipeDeadline, LifecyclePipeSession, MIN_CAPTURED_LIFECYCLE_PIPE_BYTES,
    },
};
use sugarsubstitute_supervision::{
    ClaimLaunchJobs, LaunchTicket, WindowsLaunchCommand, WindowsLaunchInput,
    profiles::{PROFILE_OPERATION_BYTES, ProfileDesktopLaunch, WindowsProfileLease},
};
type Result<T> = std::result::Result<T, Box<dyn Error>>;
/// Retain original projection charge through all profile, section and creation work.
/// The parent lifecycle actor exclusively consumes the pipe's actual terminal result.
pub(super) struct DesktopStart {
    pub(super) run: RunKey,
    pub(super) launch: LaunchId,
    pub(super) navigation: Generation,
    pub(super) connection: ConnectionId,
    pub(super) projection: JobCompletion<ProjectedEntry, ProjectionFailure>,
    pub(super) profile: WindowsProfileLease,
    pub(super) pipe: Option<LifecyclePipeSession>,
    pipe_terminal: Option<
        JobCompletion<
            sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeReport,
            sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeFailure,
        >,
    >,
    captured: Option<CapturedLifecycleStart>,
    bootstrap: Option<LifecyclePipeBootstrap>,
    enrollment: Option<LifecycleDesktopEnrollment>,
    prepared: Option<ProfileDesktopLaunch>,
    pub(super) ticket: Option<LaunchTicket>,
    pub(super) original: Option<LifecyclePipeDeadline>,
    acquired: bool,
    attempted: bool,
}
impl DesktopStart {
    /// Bind only actual projected source and native attachment pin to current authority.
    /// No key, creator identity or native source can be reconstructed from observations.
    pub(super) fn new(
        run: RunKey,
        launch: LaunchId,
        navigation: Generation,
        connection: ConnectionId,
        mut projection: JobCompletion<ProjectedEntry, ProjectionFailure>,
        pin: CaptureAttachLaunchPin,
        resources: &mut EntryResources<'_>,
    ) -> Result<Self> {
        if resources.authority.key() != Some(run.owner) || !resources.authority.serving() {
            return Err("Actual coordinator authority unavailable".into());
        }
        let value = match &mut projection.outcome {
            JobOutcome::Success(value) => value,
            _ => return Err("Actual entry projection did not succeed".into()),
        };
        let now = resources.client.now()?;
        if now >= value.until {
            return Err("Original entry deadline expired".into());
        }
        let registry =
            resources.authority.registry().ok_or("Actual process registry unavailable")?;
        let profile = WindowsProfileLease::new(run, value.requested_profile.clone(), registry)?;
        let snapshot =
            value.snapshot.take().ok_or("Original startup snapshot already transferred")?;
        Ok(Self {
            run,
            launch,
            navigation,
            connection,
            projection,
            profile,
            pipe: None,
            pipe_terminal: None,
            captured: Some(CapturedLifecycleStart::new(pin, snapshot)),
            bootstrap: None,
            enrollment: None,
            prepared: None,
            ticket: None,
            original: None,
            acquired: false,
            attempted: false,
        })
    }
    /// Admit acquisition only after this complete record is retained by Activation.
    pub(super) fn acquire_profile(&mut self, resources: &EntryResources<'_>) -> Result<()> {
        if self.acquired {
            return Ok(());
        }
        let until = self.value()?.until;
        self.profile.submit_acquire(
            resources.client,
            resources.finite,
            JobSpec {
                id: resources.jobs.allocate()?,
                operation: Operation::NativeLifecycle,
                class: JobClass::InfrastructureLifecycle,
                deadline: Some(until),
                reservation_bytes: PROFILE_OPERATION_BYTES,
                replacement: None,
            },
        )?;
        self.acquired = true;
        Ok(())
    }
    /// Reserve the actual captured source before allocating its one native bootstrap.
    pub(super) fn reserve_channel(&mut self, resources: &EntryResources<'_>) -> Result<()> {
        if self.pipe.is_some() {
            return Ok(());
        }
        let until = self.value()?.until;
        let input = self.captured.as_mut().ok_or("Original captured source missing")?;
        let spec = JobSpec {
            id: resources.jobs.allocate()?,
            operation: Operation::NativeLifecycle,
            class: JobClass::NativeObservation,
            deadline: None,
            reservation_bytes: MIN_CAPTURED_LIFECYCLE_PIPE_BYTES,
            replacement: None,
        };
        self.pipe = Some(resources.client.reserve_captured_lifecycle_pipe_server(
            resources.native,
            spec,
            self.run,
            until,
            input,
        )?);
        Ok(())
    }
    /// Retain actual bootstrap and enrollment together before any contained launch.
    pub(super) fn prepare_launch(
        &mut self,
        executable: &Path,
        working_directory: &Path,
        resources: &mut EntryResources<'_>,
    ) -> Result<bool> {
        if self.ticket.is_some() {
            return Ok(true);
        }
        if self.attempted {
            return Err("Desktop launch already attempted".into());
        }
        if !self.profile.snapshot().held || self.profile.snapshot().pending {
            return Ok(false);
        }
        let pipe = self.pipe.as_ref().ok_or("Actual captured channel missing")?;
        if self.bootstrap.is_none() {
            let Some(bootstrap) = pipe.control.take_bootstrap()? else { return Ok(false) };
            self.original = bootstrap.deadline();
            self.bootstrap = Some(bootstrap);
            self.enrollment = Some(pipe.control.take_desktop_enrollment()?);
        }
        if self.prepared.is_none() {
            let bootstrap = self.bootstrap.as_ref().ok_or("Actual bootstrap pin missing")?;
            let arguments =
                desktop_role::arguments(bootstrap, self.run, self.launch, self.navigation)
                    .map_err(super::outer)?;
            let command = WindowsLaunchCommand::from_utf16(
                &executable.as_os_str().encode_wide().collect::<Vec<_>>(),
                &working_directory.as_os_str().encode_wide().collect::<Vec<_>>(),
                &arguments,
            )?;
            let environment = self
                .value_mut()?
                .desktop_environment
                .take()
                .ok_or("Original desktop environment consumed")?;
            let bootstrap = self.bootstrap.take().ok_or("Actual bootstrap pin missing")?;
            let enrollment = self.enrollment.take().ok_or("Actual process enrollment missing")?;
            let input = WindowsLaunchInput::with_lifecycle_pipe(
                command,
                environment,
                bootstrap,
                enrollment,
            )?;
            self.prepared = Some(self.profile.prepare_desktop_launch(input)?);
        }
        let until = self.value()?.until;
        if resources.client.now()? >= until {
            return Err("Original desktop creation deadline expired".into());
        }
        let bytes = self
            .prepared
            .as_ref()
            .ok_or("Actual profile launch capsule missing")?
            .required_reservation_bytes();
        let root_watch = JobSpec {
            id: resources.jobs.allocate()?,
            operation: Operation::NativeLifecycle,
            class: JobClass::NativeObservation,
            deadline: None,
            reservation_bytes: 4096,
            replacement: None,
        };
        let creation = JobSpec {
            id: resources.jobs.allocate()?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(until),
            reservation_bytes: bytes,
            replacement: None,
        };
        let jobs = ClaimLaunchJobs {
            native_scope: ScopeSpec { id: resources.scopes.allocate()?, deadline: None },
            creation,
            root_watch,
        };
        let registry =
            resources.authority.registry_mut().ok_or("Actual process registry unavailable")?;
        let input = self.prepared.take().ok_or("Actual profile launch capsule missing")?;
        self.attempted = true;
        self.ticket = Some(self.profile.submit_desktop_launch(
            resources.client,
            registry,
            resources.finite,
            jobs,
            input,
        )?);
        Ok(true)
    }
    pub(super) fn value(&self) -> Result<&ProjectedEntry> {
        match &self.projection.outcome {
            JobOutcome::Success(value) => Ok(value),
            _ => Err("Original projection unavailable".into()),
        }
    }
    pub(super) fn value_mut(&mut self) -> Result<&mut ProjectedEntry> {
        match &mut self.projection.outcome {
            JobOutcome::Success(value) => Ok(value),
            _ => Err("Original projection unavailable".into()),
        }
    }
    /// Release only never-launched pins, then observe checked retirement of the actual pipe.
    /// Failure/panic/timeout preserves its complete terminal guard and remains unsettled.
    pub(super) fn close_unlaunched(&mut self) -> Result<bool> {
        self.prepared = None;
        self.bootstrap = None;
        self.enrollment = None;
        self.captured = None;
        let Some(pipe) = &mut self.pipe else { return Ok(true) };
        pipe.control.close();
        while pipe.control.take_observation().is_some() {}
        if self.pipe_terminal.is_none() {
            self.pipe_terminal = pipe.completion.try_take_completion();
        }
        let Some(terminal) = &self.pipe_terminal else { return Ok(false) };
        if terminal.scope_id != pipe.completion.scope_id()
            || terminal.spec != pipe.completion.spec()
        {
            return Err("Actual unlaunched pipe completion identity mismatch".into());
        }
        Ok(matches!(terminal.outcome, JobOutcome::Cancelled)
            || matches!(&terminal.outcome, JobOutcome::Success(report) if report.run == Some(self.run)))
    }
}
