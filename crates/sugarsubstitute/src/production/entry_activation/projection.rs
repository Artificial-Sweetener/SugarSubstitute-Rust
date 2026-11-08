// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Project one registered startup specification under its admitted finite allocation.
#[path = "projection/capacity.rs"]
mod capacity;
#[path = "projection/failure.rs"]
mod failure;
#[path = "projection/fields.rs"]
mod fields;
use failure::ProjectionCause;
pub(super) use failure::{ControlFailure, ProjectionFailure, check, return_unused_source};
use fields::Fields;
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
use std::{error::Error, path::PathBuf, sync::Arc};
use sugarsubstitute_contracts::{
    configuration::{BackendDataDisposition, LaunchConfiguration, LocaleId, ProfileId},
    coordinator_entry::{EntryConnectionId, EntryLaunchRequestView, EntryRequestId},
    execution::{JobContext, JobOutcome, MemoryClaim, MemoryClaimError, Moment},
    lifecycle::BackendTarget,
};
use sugarsubstitute_execution::{
    AuthenticatedEntryLaunch, ChargedBlockingInput, ExecutionClient, PreparedStartupSnapshot,
    REGISTERED_STARTUP_BYTES, RegisteredStartupSource,
};
use sugarsubstitute_supervision::{
    LaunchEnvironment,
    installed_frontend::FrontendInstallationInput,
    setup_storage::{BackendStorage, RunningApplication, SourceLaunchSettlement},
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
    user: PathBuf,
    installation: Option<Arc<RunningApplication>>,
    source: Option<Arc<SourceLaunchSettlement>>,
    cause: ProjectionCause,
    post: Option<ControlFailure>,
    _charge: Option<MemoryClaim>,
}
impl ProjectionInput {
    pub(super) fn new(
        request: AuthenticatedEntryLaunch,
        capture_root: PathBuf,
        user: PathBuf,
        client: ExecutionClient,
        run: sugarsubstitute_contracts::lifecycle::RunKey,
        installation: Option<Arc<RunningApplication>>,
    ) -> Self {
        Self {
            request: Some(request),
            client: Some(client),
            run: Some(run),
            restart: None,
            snapshot: None,
            capture_root,
            user,
            installation,
            source: None,
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
            user: PathBuf::new(),
            installation: None,
            source: None,
            cause: ProjectionCause::NotStarted,
            post: None,
            _charge: None,
        }
    }
    pub(super) fn rejected_request(self) -> Option<AuthenticatedEntryLaunch> {
        self.request
    }
    pub(super) fn authenticated_request(&self) -> Option<&AuthenticatedEntryLaunch> {
        self.request.as_ref()
    }
    pub(super) fn run_key(&self) -> Option<sugarsubstitute_contracts::lifecycle::RunKey> {
        self.run
    }
    pub(super) fn original_until(&self) -> Option<Moment> {
        self.request.as_ref().map(AuthenticatedEntryLaunch::until)
    }
    pub(super) fn release_request(&mut self) {
        drop(self.request.take());
    }
    pub(super) fn retain_source(&mut self, source: Arc<SourceLaunchSettlement>) {
        self.source = Some(source);
    }
    pub(super) fn observed_source_terminal(&mut self) -> Result<(), Box<dyn Error>> {
        let source = self
            .source
            .as_mut()
            .and_then(Arc::get_mut)
            .ok_or("Source settlement terminal has no unique retained owner")?;
        source.observed_settlement_terminal()?;
        Ok(())
    }
}
/// Keep all projected data and the original snapshot ahead of its retained charge.
/// Paths remain lexical; frontend, claims and profile owners perform actual trust checks.
pub(super) struct ProjectedEntry {
    pub(super) source: Option<Arc<SourceLaunchSettlement>>,
    pub(super) snapshot: Option<PreparedStartupSnapshot>,
    pub(super) registered: RegisteredStartupSource,
    pub(super) until: Moment,
    pub(super) entry_connection: EntryConnectionId,
    pub(super) entry_request: EntryRequestId,
    pub(super) capture_root: PathBuf,
    pub(super) requested_profile: PathBuf,
    pub(super) profile_units: Vec<u16>,
    pub(super) profile: ProfileId,
    pub(super) session_installation: sugarsubstitute_contracts::installation::InstallationId,
    pub(super) locale: LocaleId,
    pub(super) target: BackendTarget,
    pub(super) frontend: Option<FrontendInstallationInput>,
    pub(super) backend_environment: Option<LaunchEnvironment>,
    pub(super) backend_data_disposition: BackendDataDisposition,
    pub(super) desktop_environment: Option<LaunchEnvironment>,
    pub(super) policy_credit: Option<super::funding::ProjectionPolicyCredit>,
    _charge: super::funding::ProjectionDataFunding,
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
            validate_source(
                view,
                input.input().source.as_deref(),
                input.input().installation.is_some(),
            )?;
            LaunchConfiguration::validate_path(&input.input().capture_root)?;
            fields::project(
                view,
                &input.input().capture_root,
                capacity::field_available(input.input())?,
                input.input().source.as_ref().and_then(|source| source.profile_directory()),
                |configuration| match &input.input().installation {
                    Some(installation) => installation
                        .backend_storage(configuration.root(), &context)
                        .map_err(|cause| Box::new(cause) as _),
                    None => Ok(BackendStorage::External),
                },
            )
            .and_then(|fields| {
                let identity = host_identity(input.input(), &fields.profile, &context)?;
                Ok((fields, identity))
            })
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
    let (fields, session_installation) = match (projected, post) {
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
    let mut snapshot = match request.into_startup_snapshot() {
        Ok(snapshot) => snapshot,
        Err(rejected) => {
            input.input_mut().cause = ProjectionCause::Operation(Box::new(rejected.error));
            input.input_mut().request = Some(rejected.launch);
            input.input_mut().post = check(&context, received, until).err();
            return Err(ProjectionFailure { retained: input });
        }
    };
    let registered = match (&input.input().client, input.input().run) {
        (Some(client), Some(run)) => {
            client.register_startup_source(&mut snapshot, &context, run, &fields.requested_profile)
        }
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
    let (charge, policy_credit) =
        match capacity::funding(input.input_mut(), &fields, &session_installation) {
            Ok(funding) => funding,
            Err(cause) => {
                input.input_mut().snapshot = Some(snapshot);
                input.input_mut().cause = ProjectionCause::Operation(cause);
                return Err(ProjectionFailure { retained: input });
            }
        };
    Ok(ProjectedEntry {
        source: input.input_mut().source.take(),
        snapshot: Some(snapshot),
        registered,
        until,
        entry_connection,
        entry_request,
        capture_root: std::mem::take(&mut input.input_mut().capture_root),
        requested_profile: fields.requested_profile,
        profile_units: fields.profile_units,
        profile: fields.profile,
        session_installation,
        locale: fields.locale,
        target: fields.target,
        frontend: Some(fields.frontend),
        backend_environment: fields.backend_environment,
        backend_data_disposition: fields.backend_data_disposition,
        desktop_environment: Some(fields.desktop_environment),
        policy_credit: Some(policy_credit),
        _charge: charge,
    })
}
/// Inert reference bytes never substitute for the native settlement retained by this run.
fn validate_source(
    view: EntryLaunchRequestView<'_>,
    source: Option<&SourceLaunchSettlement>,
    installed: bool,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    match (installed, source, view.originals()) {
        (true, None, None) => Ok(()),
        (false, Some(source), Some(reference))
            if source.originals_reference() == Some(reference)
                && source.profile_directory().is_some() =>
        {
            Ok(())
        }
        _ => Err("Entry originals and retained installed/source settlement disagree".into()),
    }
}
#[cfg(test)]
impl super::ProductionActivation {
    /// Exercise the actual parent field projection without constructing authentication or admission.
    pub(in crate::production) fn projected_target_for_test(
        view: EntryLaunchRequestView<'_>,
    ) -> Result<BackendTarget, Box<dyn Error + Send + Sync>> {
        Ok(fields::project(view, &PathBuf::new(), u64::MAX, None, |_| {
            Ok(BackendStorage::External)
        })?
        .target)
    }
}
/// Resolve the protected session owner for this projection and require any settled source
/// launch to name the same host, so source data can never be adopted by another installation.
fn host_identity(
    input: &ProjectionInput,
    profile: &ProfileId,
    context: &JobContext,
) -> std::io::Result<sugarsubstitute_contracts::installation::InstallationId> {
    let identity = sugarsubstitute_supervision::session_identity::session_host_identity(
        &input.user,
        profile,
        input.installation.as_deref(),
        context,
    )?;
    if input.source.as_ref().is_some_and(|source| source.source_correlation() != Some(&identity)) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Settled source correlation differs from the protected session owner",
        ));
    }
    Ok(identity)
}
