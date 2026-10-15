// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Transfer actual capture and profile custody into one contained desktop launch.
use super::{
    desktop_output::DesktopOutput,
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
    qualification: crate::production::qualification::QualificationIntent,
    pub(super) run: RunKey,
    pub(super) launch: LaunchId,
    pub(super) navigation: Generation,
    pub(super) connection: ConnectionId,
    pub(super) projection: JobCompletion<ProjectedEntry, ProjectionFailure>,
    pub(super) profile: WindowsProfileLease,
    pub(super) pipe: Option<LifecyclePipeSession>,
    pub(super) output: Option<DesktopOutput>,
    pipe_terminal: Option<
        JobCompletion<
            sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeReport,
            sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeFailure,
        >,
    >,
    checked_retirement:
        Option<sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeRetirement>,
    captured: Option<CapturedLifecycleStart>,
    bootstrap: Option<LifecyclePipeBootstrap>,
    enrollment: Option<LifecycleDesktopEnrollment>,
    prepared: Option<ProfileDesktopLaunch>,
    pub(super) ticket: Option<LaunchTicket>,
    pub(super) original: Option<LifecyclePipeDeadline>,
    acquired: bool,
    publication_issued: bool,
    custody_watch: Option<(ScopeSpec, JobSpec)>,
    attempted: bool,
}
impl DesktopStart {
    /// Bind only actual projected source and native attachment pin to current authority.
    /// No key, creator identity or native source can be reconstructed from observations.
    pub(super) fn new(
        identity: sugarsubstitute_contracts::startup::MainIdentity,
        qualification: crate::production::qualification::QualificationIntent,
        connection: ConnectionId,
        mut projection: JobCompletion<ProjectedEntry, ProjectionFailure>,
        pin: CaptureAttachLaunchPin,
        resources: &mut EntryResources<'_>,
    ) -> Result<Self> {
        let sugarsubstitute_contracts::startup::MainIdentity { run, launch, navigation } = identity;
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
            qualification,
            run,
            launch,
            navigation,
            connection,
            projection,
            profile,
            pipe: None,
            output: None,
            pipe_terminal: None,
            checked_retirement: None,
            captured: Some(CapturedLifecycleStart::new(pin, snapshot)),
            bootstrap: None,
            enrollment: None,
            prepared: None,
            ticket: None,
            original: None,
            acquired: false,
            publication_issued: false,
            custody_watch: None,
            attempted: false,
        })
    }
    /// Admit acquisition only after this complete record is retained by Activation.
    pub(super) fn acquire_profile(
        &mut self,
        parent: &sugarsubstitute_supervision::BackendCustodyParent,
        resources: &mut EntryResources<'_>,
    ) -> Result<()> {
        if self.acquired {
            if !self.publication_issued && !self.profile.snapshot().pending {
                let spec = JobSpec {
                    id: resources.jobs.allocate()?,
                    operation: Operation::NativeLifecycle,
                    class: JobClass::InfrastructureLifecycle,
                    deadline: Some(self.value()?.until),
                    reservation_bytes: PROFILE_OPERATION_BYTES,
                    replacement: None,
                };
                self.publication_issued = self
                    .profile
                    .submit_custody_publication(resources.client, resources.finite, spec)?
                    .is_some();
            }
            return Ok(());
        }
        let until = self.value()?.until;
        let root_watch = JobSpec {
            id: resources.jobs.allocate()?,
            operation: Operation::NativeLifecycle,
            class: JobClass::NativeObservation,
            deadline: None,
            reservation_bytes: 4096,
            replacement: None,
        };
        let native_scope = ScopeSpec { id: resources.scopes.allocate()?, deadline: None };
        self.profile.submit_custodied_acquire(
            resources.client,
            resources.authority.registry_mut().ok_or("Actual profile registry unavailable")?,
            resources.finite,
            JobSpec {
                id: resources.jobs.allocate()?,
                operation: Operation::NativeLifecycle,
                class: JobClass::InfrastructureLifecycle,
                deadline: Some(until),
                reservation_bytes: PROFILE_OPERATION_BYTES,
                replacement: None,
            },
            parent,
            (native_scope, root_watch),
        )?;
        self.acquired = true;
        self.custody_watch = Some((native_scope, root_watch));
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
        if !self.profile.custody_publication_ready()? {
            return Ok(false);
        }
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
        let pipe = self.pipe.as_ref().ok_or("Actual captured channel missing")?;
        if self.bootstrap.is_none() {
            let Some(bootstrap) = pipe.control.take_bootstrap()? else {
                return Ok(false);
            };
            self.original = bootstrap.deadline();
            self.bootstrap = Some(bootstrap);
            self.enrollment = Some(pipe.control.take_desktop_enrollment()?);
        }
        if self.prepared.is_none() {
            let bootstrap = self.bootstrap.as_ref().ok_or("Actual bootstrap pin missing")?;
            let arguments = desktop_role::arguments(
                bootstrap,
                self.run,
                self.launch,
                self.navigation,
                &self.qualification,
            )
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
            let input =
                self.output.as_mut().ok_or("Actual desktop output missing")?.attach(input)?;
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
        let (native_scope, root_watch) =
            self.custody_watch.ok_or("Original prepared profile watch absent")?;
        let creation = JobSpec {
            id: resources.jobs.allocate()?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(until),
            reservation_bytes: bytes,
            replacement: None,
        };
        let jobs = ClaimLaunchJobs { native_scope, creation, root_watch };
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
    /// Every outcome retains its complete guard; only an independent checked proof permits settlement.
    pub(super) fn close_unlaunched(&mut self) -> Result<bool> {
        self.prepared = None;
        self.bootstrap = None;
        self.enrollment = None;
        self.captured = None;
        let output_settled = if let Some(output) = &mut self.output {
            output.request_close();
            output.observe(
                #[cfg(feature = "k1-native-qualification")]
                None,
            )?;
            output.settled()
        } else {
            true
        };
        let Some(pipe) = &mut self.pipe else {
            return Ok(output_settled);
        };
        pipe.control.close();
        while pipe.control.take_observation().is_some() {}
        if self.pipe_terminal.is_none() {
            self.pipe_terminal = pipe.completion.try_take_completion();
        }
        if self.checked_retirement.is_none() {
            self.checked_retirement = pipe.control.take_checked_retirement();
        }
        let Some(terminal) = &self.pipe_terminal else {
            return Ok(false);
        };
        if terminal.scope_id != pipe.completion.scope_id()
            || terminal.spec != pipe.completion.spec()
        {
            return Err("Actual unlaunched pipe completion identity mismatch".into());
        }
        // The complete original outcome remains retained independently of physical settlement.
        // A failure terminal cannot substitute for the actual checked worker proof.
        Ok(output_settled
            && self.checked_retirement.as_ref().is_some_and(|proof| {
                proof.matches(&pipe.control)
                    && proof.scope_id() == terminal.scope_id
                    && proof.spec() == terminal.spec
                    && proof.run() == Some(self.run)
            }))
    }
}
