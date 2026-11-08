// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Convert the authenticated startup loan on the existing finite worker owner.
//! Prepared application intent retains the original mapping and its allocation claim.
use std::{
    collections::TryReserveError,
    error::Error,
    ffi::OsString,
    fmt,
    os::windows::ffi::OsStringExt,
    path::{Path, PathBuf},
};
use sugarsubstitute_application::startup::StartupSession;
use sugarsubstitute_contracts::{
    configuration::{
        BackendDataDisposition, ConfigurationError, ConnectionConfiguration, LaunchConfiguration,
        ProfileId, StartupConfiguration,
    },
    coordinator_entry::{EntryConnectionView, EntryLaunchRequestView, EntryWindowsPathView},
    execution::{
        AdmissionError, ExecutionPort, Generation, JobClass, JobContext, JobId, JobSpec,
        MemoryClaim, MemoryClaimError, MemoryLease, Moment, Operation, OwnedJob, Scope,
    },
    startup::LaunchId,
};
use sugarsubstitute_execution::{
    EntryTransportError, ExecutionClient, windows_lifecycle_pipe::AuthenticatedStartupSnapshot,
};

/// Finite startup projection capacity, independent of the permanent capture reservation.
pub(super) const RESERVATION_BYTES: u64 = 1024 * 1024;
const PROJECTION_BYTES: u64 = 512 * 1024;
// Cover simultaneous UTF-16 staging and owned path storage at the wire's 32766-unit bound.
const MAIN_INPUT_BYTES: u64 = 192 * 1024;

/// Transfer only charged profile preparation data to the later finite main-window worker.
pub(in crate::production) struct PreparedMainInput {
    directory: PathBuf,
    profile: ProfileId,
    _allocation: MemoryClaim,
}
impl PreparedMainInput {
    /// Borrow the exact requested path; this grants no filesystem authorization.
    pub(in crate::production) fn directory(&self) -> &Path {
        &self.directory
    }
    /// Borrow the authenticated requested profile spelling without normalizing it.
    pub(in crate::production) fn profile(&self) -> &ProfileId {
        &self.profile
    }
}

/// Retain application state before its source loan and reusable allocation charge.
pub(crate) struct PreparedChildStartup {
    session: StartupSession,
    main: Option<PreparedMainInput>,
    source: Option<AuthenticatedStartupSnapshot>,
    _allocation: MemoryClaim,
}

/// Transfer only sendable configuration from the worker; native-thread session follows.
pub(super) struct PreparedChildConfiguration {
    configuration: StartupConfiguration,
    main: PreparedMainInput,
    source: Option<AuthenticatedStartupSnapshot>,
    _allocation: MemoryClaim,
}
impl PreparedChildConfiguration {
    /// Construct native-thread application policy while moving both original guards.
    /// This grants no capture, connection or frontend readiness.
    pub(super) fn into_startup(self, identity: (LaunchId, Generation)) -> PreparedChildStartup {
        PreparedChildStartup {
            session: StartupSession::new(identity.0, identity.1, self.configuration),
            main: Some(self.main),
            source: self.source,
            _allocation: self._allocation,
        }
    }
    /// Return only the map loan after native UI retirement; retain all charged data.
    pub(super) fn release_source(&mut self) {
        self.source = None;
    }
}
impl PreparedChildStartup {
    /// Transfer the original charged main input once without moving native-thread policy.
    pub(in crate::production) fn take_main_input(&mut self) -> Option<PreparedMainInput> {
        self.main.take()
    }
    /// Restore the identical charged main data after actual attempt retirement.
    /// # Errors
    /// Rejects replacement of an existing original input.
    pub(in crate::production) fn restore_main_input(
        &mut self,
        input: &mut Option<PreparedMainInput>,
    ) -> Result<(), Box<dyn Error>> {
        if input.is_none() {
            return Ok(());
        }
        if self.main.is_some() {
            return Err("Original main input remains retained".into());
        }
        self.main = input.take();
        Ok(())
    }
    /// Borrow the acknowledged application startup owner without detaching its charge.
    pub(in crate::production) fn session(&self) -> &StartupSession {
        &self.session
    }
    /// Mutate application policy while the authenticated source remains retained.
    pub(in crate::production) fn session_mut(&mut self) -> &mut StartupSession {
        &mut self.session
    }
    /// Borrow unchanged input for the native profile/frontend owners.
    pub(in crate::production) fn source(&self) -> Option<&AuthenticatedStartupSnapshot> {
        self.source.as_ref()
    }
    /// Return the original map loan after the composing owner observes native retirement.
    /// Application state and its allocation claim remain owned through executor joins.
    /// Returning the loan permits its native owner to attempt checked unmapping;
    /// this method itself establishes no physical transport settlement.
    pub(in crate::production) fn release_source(&mut self) {
        self.source = None;
    }
}

/// Preserve projection failure and the same authenticated mapping loan.
pub(super) struct ChildConfigurationFailure {
    cause: ProjectionError,
    source: Option<AuthenticatedStartupSnapshot>,
}
impl ChildConfigurationFailure {
    /// Return the failed projection's loan after observed native UI retirement.
    pub(super) fn release_source(&mut self) {
        self.source = None;
    }
}
impl fmt::Debug for ChildConfigurationFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ChildConfigurationFailure { retained, redacted }")
    }
}
impl fmt::Display for ChildConfigurationFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.cause.fmt(f)
    }
}
impl Error for ChildConfigurationFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.cause)
    }
}

#[derive(Debug)]
pub(super) enum ProjectionError {
    Source(EntryTransportError),
    Configuration(ConfigurationError),
    /// Preserve the sole Comfy owner's complete-command policy rejection.
    ManagedLaunch(sugarsubstitute_comfy::launch::ManagedLaunchError),
    Capacity(TryReserveError),
    Memory(MemoryClaimError),
    Cancelled,
    Deadline,
    Disposition,
}
impl fmt::Display for ProjectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Authenticated startup configuration could not be prepared")
    }
}
impl Error for ProjectionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Source(cause) => Some(cause),
            Self::Configuration(cause) => Some(cause),
            Self::ManagedLaunch(cause) => Some(cause),
            Self::Capacity(cause) => Some(cause),
            Self::Memory(cause) => Some(cause),
            Self::Cancelled | Self::Deadline | Self::Disposition => None,
        }
    }
}

/// Submit bounded conversion through the child's existing execution owner.
///
/// The successful typed result retains source and allocation custody together.
/// Rejected admission returns the actual cause before conversion or native effects;
/// the moved mapping loan returns to its native owner when the rejected task drops.
/// # Errors
/// Rejects the actual scope/specification or bounded queue/memory admission.
pub(super) fn submit(
    client: &ExecutionClient,
    scope: &Scope,
    id: JobId,
    source: AuthenticatedStartupSnapshot,
    disposition: BackendDataDisposition,
    original: Moment,
) -> Result<OwnedJob<PreparedChildConfiguration, ChildConfigurationFailure>, AdmissionError> {
    client.submit_blocking(scope, request(id, original), move |context| {
        let prepared = (|| {
            checkpoint(&context)?;
            let claim =
                context.memory().claim(PROJECTION_BYTES).map_err(ProjectionError::Memory)?;
            let view = source.view().map_err(ProjectionError::Source)?;
            let configuration = project(view, disposition)?;
            let main = project_main(view, context.memory())?;
            checkpoint(&context)?;
            Ok::<_, ProjectionError>((configuration, main, claim))
        })();
        match prepared {
            Ok((configuration, main, claim)) => Ok(PreparedChildConfiguration {
                configuration,
                main,
                source: Some(source),
                _allocation: claim,
            }),
            Err(cause) => Err(ChildConfigurationFailure { cause, source: Some(source) }),
        }
    })
}

/// Keep finite conversion on its original reserved lifecycle worker capacity.
pub(super) fn request(id: JobId, original: Moment) -> JobSpec {
    JobSpec {
        id,
        operation: Operation::Infrastructure,
        class: JobClass::InfrastructureLifecycle,
        deadline: Some(original),
        reservation_bytes: RESERVATION_BYTES,
        replacement: None,
    }
}

fn checkpoint(context: &JobContext) -> Result<(), ProjectionError> {
    if context.cancellation_requested() {
        return Err(ProjectionError::Cancelled);
    }
    if context.deadline().is_none_or(|until| context.now() >= until) {
        return Err(ProjectionError::Deadline);
    }
    Ok(())
}

fn project(
    view: EntryLaunchRequestView<'_>,
    disposition: BackendDataDisposition,
) -> Result<StartupConfiguration, ProjectionError> {
    if disposition != BackendDataDisposition::External {
        return Err(ProjectionError::Disposition);
    }
    let connection = match view.connection() {
        EntryConnectionView::Attached(origin)
            if disposition == BackendDataDisposition::External =>
        {
            ConnectionConfiguration::Attach(origin)
        }
        EntryConnectionView::Attached(_) => return Err(ProjectionError::Disposition),
        EntryConnectionView::Managed { root, interpreter, arguments, port } => {
            let root = path(root)?;
            let interpreter = path(interpreter)?;
            let mut owned = Vec::new();
            owned.try_reserve_exact(arguments.len()).map_err(ProjectionError::Capacity)?;
            for argument in arguments.iter() {
                owned.push(text(argument)?);
            }
            let requested = LaunchConfiguration::new(root, interpreter, owned, port.get())
                .map_err(ProjectionError::Configuration)?;
            // Match the independently projected parent's semantic target using
            // the same sole option owner; the original projection claim covers both values.
            ConnectionConfiguration::Launch(
                sugarsubstitute_comfy::launch::effective_managed_launch(&requested)
                    .map_err(ProjectionError::ManagedLaunch)?,
            )
        }
    };
    let profile = ProfileId::new(text(view.profile())?).map_err(ProjectionError::Configuration)?;
    Ok(StartupConfiguration::new(connection, profile, view.presentation())
        .with_canvas_renderer(view.canvas_renderer().to_owned_preference()))
}

pub(super) fn project_main(
    view: EntryLaunchRequestView<'_>,
    memory: &MemoryLease,
) -> Result<PreparedMainInput, ProjectionError> {
    let claim = memory.claim(MAIN_INPUT_BYTES).map_err(ProjectionError::Memory)?;
    let directory = path(view.profile_directory())?;
    let profile = ProfileId::new(text(view.profile())?).map_err(ProjectionError::Configuration)?;
    Ok(PreparedMainInput { directory, profile, _allocation: claim })
}

fn text(value: &str) -> Result<String, ProjectionError> {
    let mut owned = String::new();
    owned.try_reserve_exact(value.len()).map_err(ProjectionError::Capacity)?;
    owned.push_str(value);
    Ok(owned)
}

fn path(value: EntryWindowsPathView<'_>) -> Result<PathBuf, ProjectionError> {
    let mut units = Vec::new();
    units.try_reserve_exact(value.unit_len()).map_err(ProjectionError::Capacity)?;
    units.extend(value.units());
    Ok(PathBuf::from(OsString::from_wide(&units)))
}

#[cfg(test)]
#[path = "configuration/tests.rs"]
pub(super) mod tests;
