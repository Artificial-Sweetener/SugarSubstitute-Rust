// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Project one registered startup specification under its admitted finite allocation.
#[path = "projection/capacity.rs"]
mod capacity;
#[path = "projection/receiving.rs"]
mod receiving;
#[path = "projection/restart.rs"]
mod restart;
pub(super) use restart::RestartProjection;
#[cfg(test)]
#[path = "projection_retirement_tests.rs"]
mod retirement_tests;
#[cfg(test)]
#[path = "projection_tests.rs"]
mod tests;
use std::{error::Error, ffi::OsString, fmt, os::windows::ffi::OsStringExt, path::PathBuf};
use sugarsubstitute_contracts::{
    configuration::{LaunchConfiguration, LocaleId, ProfileId},
    coordinator_entry::{
        EntryConnectionId, EntryConnectionView, EntryLaunchRequestView, EntryRequestId,
        EntryWindowsPathView,
    },
    execution::{JobContext, JobOutcome, MemoryClaim, MemoryClaimError, Moment},
    lifecycle::BackendTarget,
};
use sugarsubstitute_execution::{
    AuthenticatedEntryLaunch, ChargedBlockingInput, ExecutionClient, PreparedStartupSnapshot,
    REGISTERED_STARTUP_BYTES, RegisteredStartupSource,
};
use sugarsubstitute_supervision::{
    LaunchEnvironment, installed_frontend::FrontendInstallationInput,
};

/// Bound known projection buffers; the original frame keeps its own native loan.
pub(super) const PROJECTION_BYTES: u64 = 4 * 1024 * 1024;
// Keep co-resident lifecycle storage separate from the projected data claim.
pub(super) const fn projection_data_bytes() -> Option<u64> {
    let bytes = match PROJECTION_BYTES.checked_sub(super::ParentLifecycle::storage_bytes()) {
        Some(bytes) => bytes,
        None => return None,
    };
    let bytes = match bytes.checked_sub(size_of::<super::Record>() as u64) {
        Some(bytes) => bytes,
        None => return None,
    };
    let bytes = match bytes.checked_sub(super::RetainedBackend::required_storage_bytes()) {
        Some(bytes) => bytes,
        None => return None,
    };
    let bytes = match bytes.checked_sub(super::delivery::TransitionDelivery::storage_bytes()) {
        Some(bytes) => bytes,
        None => return None,
    };
    // The original worker input retires before this same reservation can host recovery.
    let input_bytes = size_of::<ProjectionInput>();
    let recovery_bytes = size_of::<super::recovery_supervisor::RecoveryPreparation>();
    let input_or_recovery_bytes =
        if input_bytes > recovery_bytes { input_bytes } else { recovery_bytes };
    let bytes = match bytes.checked_sub(input_or_recovery_bytes as u64) {
        Some(bytes) => bytes,
        None => return None,
    };
    bytes.checked_sub(REGISTERED_STARTUP_BYTES)
}
/// Keep the real frame and configured inert capture location across admission rejection.
pub(super) struct ProjectionInput {
    request: Option<AuthenticatedEntryLaunch>,
    client: Option<ExecutionClient>,
    run: Option<sugarsubstitute_contracts::lifecycle::RunKey>,
    restart: Option<RestartProjection>,
    snapshot: Option<PreparedStartupSnapshot>,
    capture_root: PathBuf,
    cause: ProjectionCause,
    post: Option<ControlFailure>,
    _charge: Option<MemoryClaim>,
}
impl ProjectionInput {
    pub(super) fn new(
        request: AuthenticatedEntryLaunch,
        capture_root: PathBuf,
        client: ExecutionClient,
        run: sugarsubstitute_contracts::lifecycle::RunKey,
    ) -> Self {
        Self {
            request: Some(request),
            client: Some(client),
            run: Some(run),
            restart: None,
            snapshot: None,
            capture_root,
            cause: ProjectionCause::NotStarted,
            post: None,
            _charge: None,
        }
    }
    /// Supply inert input to the real admitted failure path without an authenticated loan.
    #[cfg(test)]
    pub(super) fn without_request() -> Self {
        Self {
            request: None,
            client: None,
            run: None,
            restart: None,
            snapshot: None,
            capture_root: PathBuf::new(),
            cause: ProjectionCause::NotStarted,
            post: None,
            _charge: None,
        }
    }
    pub(super) fn rejected_request(self) -> Option<AuthenticatedEntryLaunch> {
        self.request
    }
}
/// Keep all projected data and the original snapshot ahead of its retained charge.
/// Paths remain lexical; frontend, claims and profile owners perform actual trust checks.
pub(super) struct ProjectedEntry {
    pub(super) snapshot: Option<PreparedStartupSnapshot>,
    pub(super) registered: RegisteredStartupSource,
    pub(super) until: Moment,
    pub(super) entry_connection: EntryConnectionId,
    pub(super) entry_request: EntryRequestId,
    pub(super) capture_root: PathBuf,
    pub(super) requested_profile: PathBuf,
    pub(super) profile_units: Vec<u16>,
    pub(super) profile: ProfileId,
    pub(super) locale: LocaleId,
    pub(super) presentation: sugarsubstitute_contracts::configuration::Presentation,
    pub(super) target: BackendTarget,
    pub(super) frontend: Option<FrontendInstallationInput>,
    pub(super) backend_environment: Option<LaunchEnvironment>,
    pub(super) desktop_environment: Option<LaunchEnvironment>,
    pub(super) policy_credit: Option<super::funding::ProjectionPolicyCredit>,
    _charge: super::funding::ProjectionDataFunding,
}

/// Preserve the original failure and frame through late cancellation or rejection.
pub(super) struct ProjectionFailure {
    retained: ChargedBlockingInput<ProjectionInput>,
}
/// Return a completed, unlaunched request's passive connection loan during
/// shutdown. The complete outcome, first error, controls, metadata and original
/// charges remain with the caller. This proves no transport/native settlement.
/// An admitted captured session owns its transferred source and is excluded.
pub(super) fn return_unused_source(outcome: &mut JobOutcome<ProjectedEntry, ProjectionFailure>) {
    match outcome {
        JobOutcome::Success(value) => drop(value.snapshot.take()),
        JobOutcome::Failure(failure) => {
            let input = failure.retained.input_mut();
            drop(input.request.take());
            drop(input.snapshot.take());
        }
        _ => {}
    }
}

enum ProjectionCause {
    NotStarted,
    Claim(MemoryClaimError),
    Funding(sugarsubstitute_contracts::lifecycle::LifecycleRejection),
    Operation(Box<dyn Error + Send + Sync>),
}
impl ProjectionCause {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::NotStarted => None,
            Self::Claim(cause) => Some(cause),
            Self::Funding(cause) => Some(cause),
            Self::Operation(cause) => Some(cause.as_ref()),
        }
    }
}
impl fmt::Debug for ProjectionFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EntryProjectionFailure")
            .field("post_control", &self.retained.input().post)
            .field("original_retained", &self.retained.input().request.is_some())
            .finish_non_exhaustive()
    }
}
impl fmt::Display for ProjectionFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Authenticated entry preparation failed")
    }
}
impl Error for ProjectionFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.retained.input().cause.source()
    }
}
#[derive(Debug)]
pub(super) enum ControlFailure {
    Cancelled,
    Expired,
    ClockRegression,
}
impl fmt::Display for ControlFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Entry projection control: {self:?}")
    }
}
impl Error for ControlFailure {}
fn check(context: &JobContext, received: Moment, until: Moment) -> Result<(), ControlFailure> {
    let now = context.now();
    if context.cancellation_requested() {
        return Err(ControlFailure::Cancelled);
    }
    if now < received {
        return Err(ControlFailure::ClockRegression);
    }
    if now >= until || context.deadline().is_none_or(|deadline| now >= deadline || deadline > until)
    {
        return Err(ControlFailure::Expired);
    }
    Ok(())
}

/// Execute only from the existing typed finite worker after its real admission.
/// The caller retains this complete result/completion until all transferred data retire.
pub(super) fn project(
    mut input: ChargedBlockingInput<ProjectionInput>,
    context: JobContext,
) -> Result<ProjectedEntry, ProjectionFailure> {
    let Some(bytes) = projection_data_bytes() else {
        input.input_mut().cause = ProjectionCause::Funding(
            sugarsubstitute_contracts::lifecycle::LifecycleRejection::Overloaded,
        );
        if let Some(request) = &input.input().request {
            let post = check(&context, request.received_at(), request.until()).err();
            input.input_mut().post = post;
        }
        return Err(ProjectionFailure { retained: input });
    };
    let charge = match context.memory().claim(bytes) {
        Ok(charge) => charge,
        Err(cause) => {
            input.input_mut().cause = ProjectionCause::Claim(cause);
            if let Some(request) = &input.input().request {
                let post = check(&context, request.received_at(), request.until()).err();
                input.input_mut().post = post;
            }
            return Err(ProjectionFailure { retained: input });
        }
    };
    input.input_mut()._charge = Some(charge);
    if input.input().restart.is_some() {
        return restart::project(input, context);
    }
    let Some(request) = input.input_mut().request.take() else {
        return Err(ProjectionFailure { retained: input });
    };
    let entry_connection = request.peer().connection();
    let entry_request = request.request_id();
    let received = request.received_at();
    let until = request.until();
    let outcome = check(&context, received, until)
        .map_err(|cause| Box::new(cause) as _)
        .and_then(|()| request.view().map_err(|cause| Box::new(cause) as _))
        .and_then(|view| {
            LaunchConfiguration::validate_path(&input.input().capture_root)?;
            fields(view, &input.input().capture_root, bytes)
        });
    // The same preclaimed slot adopts the primary before any lazy provider.
    let projected = match outcome {
        Ok(fields) => Some(fields),
        Err(cause) => {
            input.input_mut().cause = ProjectionCause::Operation(cause);
            None
        }
    };
    let post = check(&context, received, until).err();
    let fields = match (projected, post) {
        (Some(fields), None) => fields,
        (Some(_), Some(cause)) => {
            input.input_mut().cause = ProjectionCause::Operation(Box::new(cause));
            input.input_mut().request = Some(request);
            return Err(ProjectionFailure { retained: input });
        }
        (None, post) => {
            input.input_mut().post = post;
            input.input_mut().request = Some(request);
            return Err(ProjectionFailure { retained: input });
        }
    };
    let snapshot = match request.into_startup_snapshot() {
        Ok(snapshot) => snapshot,
        Err(rejected) => {
            input.input_mut().cause = ProjectionCause::Operation(Box::new(rejected.error));
            input.input_mut().request = Some(rejected.launch);
            input.input_mut().post = check(&context, received, until).err();
            return Err(ProjectionFailure { retained: input });
        }
    };
    let registered = match (&input.input().client, input.input().run) {
        (Some(client), Some(run)) => client.register_startup_source(&snapshot, &context, run),
        _ => Err(sugarsubstitute_contracts::execution::AdmissionError::InvalidCaptureAdmission),
    };
    let registered = match registered {
        Ok(source) => source,
        Err(cause) => {
            input.input_mut().snapshot = Some(snapshot);
            input.input_mut().cause = ProjectionCause::Operation(Box::new(cause));
            return Err(ProjectionFailure { retained: input });
        }
    };
    let (charge, policy_credit) = match capacity::funding(input.input_mut(), &fields) {
        Ok(funding) => funding,
        Err(cause) => {
            input.input_mut().snapshot = Some(snapshot);
            input.input_mut().cause = ProjectionCause::Operation(cause);
            return Err(ProjectionFailure { retained: input });
        }
    };
    Ok(ProjectedEntry {
        snapshot: Some(snapshot),
        registered,
        until,
        entry_connection,
        entry_request,
        capture_root: std::mem::take(&mut input.input_mut().capture_root),
        requested_profile: fields.requested_profile,
        profile_units: fields.profile_units,
        profile: fields.profile,
        locale: fields.locale,
        presentation: fields.presentation,
        target: fields.target,
        frontend: Some(fields.frontend),
        backend_environment: fields.backend_environment,
        desktop_environment: Some(fields.desktop_environment),
        policy_credit: Some(policy_credit),
        _charge: charge,
    })
}
struct Fields {
    allocated_bytes: u64,
    requested_profile: PathBuf,
    profile_units: Vec<u16>,
    profile: ProfileId,
    locale: LocaleId,
    presentation: sugarsubstitute_contracts::configuration::Presentation,
    target: BackendTarget,
    frontend: FrontendInstallationInput,
    backend_environment: Option<LaunchEnvironment>,
    desktop_environment: LaunchEnvironment,
}
#[cfg(test)]
impl super::ProductionActivation {
    /// Exercise the actual parent field projection without constructing authentication or admission.
    pub(in crate::production) fn projected_target_for_test(
        view: EntryLaunchRequestView<'_>,
    ) -> Result<BackendTarget, Box<dyn Error + Send + Sync>> {
        Ok(fields(view, &PathBuf::new(), u64::MAX)?.target)
    }
}
fn fields(
    view: EntryLaunchRequestView<'_>,
    capture: &PathBuf,
    available: u64,
) -> Result<Fields, Box<dyn Error + Send + Sync>> {
    capacity::preflight(view, capture, available)?;
    let profile_units = collect_units(view.profile_directory())?;
    let requested_profile = PathBuf::from(OsString::from_wide(&profile_units));
    LaunchConfiguration::validate_path(&requested_profile)?;
    let profile = ProfileId::new(copy_string(view.profile())?)?;
    let frontend = view.frontend();
    let frontend_paths = [
        path(frontend.package_directory())?,
        path(frontend.metadata_file())?,
        path(frontend.cube_directory())?,
        path(frontend.backend_script())?,
    ];
    let frontend_bytes = capacity::frontend(&frontend_paths)?;
    let [package, metadata, cube, script] = frontend_paths;
    let frontend = FrontendInstallationInput::new(package, metadata, cube, script)?;
    let target = match view.connection() {
        EntryConnectionView::Attached(origin) => BackendTarget::Attached(origin),
        EntryConnectionView::Managed { root, interpreter, arguments, port } => {
            let mut owned = Vec::new();
            owned.try_reserve_exact(arguments.len())?;
            for argument in arguments.iter() {
                owned.push(copy_string(argument)?);
            }
            let requested =
                LaunchConfiguration::new(path(root)?, path(interpreter)?, owned, port.get())?;
            // Schema-one arguments are extras. Resolve identity from the sole
            // Comfy owner's complete command, while the original data claim is held.
            BackendTarget::Managed(sugarsubstitute_comfy::launch::effective_managed_launch(
                &requested,
            )?)
        }
    };
    let mut environment = Vec::new();
    environment.try_reserve_exact(view.environment().unit_len())?;
    environment.extend(view.environment().units());
    let desktop_environment = LaunchEnvironment::from_snapshot_utf16(&environment, &[])?;
    let backend_environment = match &target {
        BackendTarget::Managed(configuration) => {
            Some(LaunchEnvironment::for_managed_backend(&environment, configuration)?)
        }
        BackendTarget::Attached(_) => None,
    };
    let mut fields = Fields {
        allocated_bytes: 0,
        requested_profile,
        profile_units,
        profile,
        locale: view.presentation().locale(),
        presentation: view.presentation(),
        target,
        frontend,
        backend_environment,
        desktop_environment,
    };
    fields.allocated_bytes = capacity::data(&fields, frontend_bytes)?;
    Ok(fields)
}
fn copy_string(value: &str) -> Result<String, std::collections::TryReserveError> {
    let mut result = String::new();
    result.try_reserve_exact(value.len())?;
    result.push_str(value);
    Ok(result)
}
fn collect_units(
    value: EntryWindowsPathView<'_>,
) -> Result<Vec<u16>, std::collections::TryReserveError> {
    let mut units = Vec::new();
    units.try_reserve_exact(value.unit_len())?;
    units.extend(value.units());
    Ok(units)
}
fn path(value: EntryWindowsPathView<'_>) -> Result<PathBuf, Box<dyn Error + Send + Sync>> {
    let units = collect_units(value)?;
    let result = PathBuf::from(OsString::from_wide(&units));
    LaunchConfiguration::validate_path(&result)?;
    Ok(result)
}
