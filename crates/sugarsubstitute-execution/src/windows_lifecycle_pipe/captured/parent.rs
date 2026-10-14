// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Move genuine parent capture and Entry custody into the sole captured native section.
use super::AdmissionBinding;
use crate::{
    ExecutionClient,
    admission::RequestKind,
    capture_attach::{CaptureAttachLaunchPin, types::CaptureEndpointObservation},
    coordinator_entry::PreparedStartupSnapshot,
    lifecycle_pipe::{self, LifecyclePipeSession, Owner, PipeAdmission, PreparedPipeLease},
    native::Signal,
    scope::lock,
    windows_lifecycle_pipe::{
        captured_layout::CapturedLayout, native, startup_mapping::StartupMapping,
    },
};
use sugarsubstitute_contracts::{
    execution::{AdmissionError, JobContext, JobSpec, Moment, Scope},
    lifecycle::RunKey,
};

/// Retain the original nonclone pin and authenticated Entry frame across rejected admission.
/// Reservation borrows this owner, taking both fields only after the final genuine fence.
/// Construction allocates no payload and creates no native or installation authority.
pub struct CapturedLifecycleStart {
    owners: Option<(CaptureAttachLaunchPin, PreparedStartupSnapshot)>,
}
impl CapturedLifecycleStart {
    /// Transfer the two already authenticated owners without copying the original frame.
    pub fn new(pin: CaptureAttachLaunchPin, startup: PreparedStartupSnapshot) -> Self {
        Self { owners: Some((pin, startup)) }
    }
    /// Observe whether admission has consumed this exact pair; this is not readiness proof.
    pub fn is_consumed(&self) -> bool {
        self.owners.is_none()
    }
}

/// Parent native lifetime retains both original owners until checked section/pipe retirement.
pub(in crate::windows_lifecycle_pipe) struct ParentSource {
    pub(in crate::windows_lifecycle_pipe) pin: CaptureAttachLaunchPin,
    pub(in crate::windows_lifecycle_pipe) startup: Option<PreparedStartupSnapshot>,
    pub(in crate::windows_lifecycle_pipe) binding: AdmissionBinding,
    pub(in crate::windows_lifecycle_pipe) endpoint: CaptureEndpointObservation,
    pub(in crate::windows_lifecycle_pipe) processes: [usize; 2],
}
impl ParentSource {
    pub(in crate::windows_lifecycle_pipe) fn body(
        &self,
    ) -> Result<&[u8], lifecycle_pipe::LifecyclePipeFailure> {
        match &self.startup {
            Some(startup) => startup.body().map_err(|cause| {
                lifecycle_pipe::LifecyclePipeFailure::native(
                    crate::native::NativeObservationError::new("captured_startup_body", cause),
                )
            }),
            None => Ok(&[]),
        }
    }

    /// Revalidate original eligibility after the external clock callback, outside native locks.
    pub(in crate::windows_lifecycle_pipe) fn check(
        &self,
        context: &JobContext,
    ) -> Result<(), lifecycle_pipe::LifecyclePipeFailure> {
        let now = context.now();
        let core = self
            .pin
            .shared
            .custody
            .core
            .upgrade()
            .ok_or_else(|| lifecycle_pipe::failure("captured_parent_owner"))?;
        let state = lock(&core.state);
        self.binding.validate_current_locked(&core, &state, now).map(|_| ()).map_err(|cause| {
            lifecycle_pipe::LifecyclePipeFailure::native(
                crate::native::NativeObservationError::new("captured_parent_current", cause),
            )
        })
    }
}

impl ExecutionClient {
    /// Admit the sole captured startup section from genuine capture/Entry owners.
    /// The persistent transport keeps `deadline=None`; its original finite handshake
    /// is narrowed under the final Core fence before IDs, capacity and construction.
    /// # Errors
    /// Returns the original cause and leaves `input` unchanged on every rejection.
    pub fn reserve_captured_lifecycle_pipe_server(
        &self,
        scope: &Scope,
        spec: JobSpec,
        run: RunKey,
        original_handshake: Moment,
        input: &mut CapturedLifecycleStart,
    ) -> Result<LifecyclePipeSession, AdmissionError> {
        let (pin, startup) =
            input.owners.as_ref().ok_or(AdmissionError::InvalidCaptureAdmission)?;
        let startup_binding = startup
            .admission_binding()
            .map_err(|cause| AdmissionError::Unexpected(crate::runtime::error(cause)))?;
        let binding = AdmissionBinding::project(run, original_handshake, pin, startup_binding);
        let layout =
            layout().map_err(|cause| AdmissionError::Unexpected(crate::runtime::error(cause)))?;
        let factory_binding = binding.clone();
        let (inner, completion) = lifecycle_pipe::admit_owned(
            &self.core,
            scope,
            spec,
            PipeAdmission {
                binding: Some(lifecycle_pipe::PeerBinding::Desktop(run)),
                handshake: original_handshake,
                owner: Owner::Desktop,
            },
            RequestKind::CapturedLifecycle(&binding),
            input,
            |input, context| build(&self.core, input, context, run, factory_binding, layout),
        )
        .map_err(|(cause, _input)| cause)?;
        Ok(LifecyclePipeSession {
            control: lifecycle_pipe::LifecyclePipeControl { inner },
            completion,
        })
    }
}

fn build<'a>(
    core: &std::sync::Arc<crate::admission::Core>,
    input: &'a mut CapturedLifecycleStart,
    context: &JobContext,
    run: RunKey,
    binding: AdmissionBinding,
    layout: CapturedLayout,
) -> Result<PreparedPipeLease, (AdmissionError, &'a mut CapturedLifecycleStart)> {
    let native_claim = match context.memory().claim(layout.native_bytes as u64) {
        Ok(claim) => claim,
        Err(cause) => {
            return Err((AdmissionError::Unexpected(crate::runtime::error(cause)), input));
        }
    };
    let map_claim = match context.memory().claim(layout.map_bytes as u64) {
        Ok(claim) => claim,
        Err(cause) => {
            return Err((AdmissionError::Unexpected(crate::runtime::error(cause)), input));
        }
    };
    let (endpoint, processes) = match input.owners.as_ref().and_then(|(pin, _)| {
        let state = lock(&pin.shared.state);
        Some((state.endpoint?, state.handles?))
    }) {
        Some(value) => value,
        None => return Err((AdmissionError::InvalidCaptureAdmission, input)),
    };
    // All allocation claims are retained before these exact native owner constructors.
    let mut lease = native::Lease::server(run);
    if let Err(cause) =
        lease.install_captured_mapping(map_claim, Signal::new(core), layout.map_extent)
    {
        return Err((AdmissionError::Unexpected(crate::runtime::error(cause)), input));
    }
    let Some((pin, startup)) = input.owners.take() else {
        return Err((AdmissionError::InvalidCaptureAdmission, input));
    };
    lease.captured_parent =
        Some(ParentSource { pin, startup: Some(startup), binding, endpoint, processes });
    Ok(PreparedPipeLease { lease: Box::new(lease), metadata: Some(native_claim) })
}

/// Sample only the actual platform page fact outside admission/lifetime guards.
#[allow(unsafe_code)]
pub(in crate::windows_lifecycle_pipe) fn layout()
-> Result<CapturedLayout, lifecycle_pipe::LifecyclePipeFailure> {
    let mut facts = windows::Win32::System::SystemInformation::SYSTEM_INFO::default();
    // SAFETY: the OS writes this live fixed output; no handles or borrowed callbacks escape.
    unsafe { windows::Win32::System::SystemInformation::GetSystemInfo(&mut facts) };
    CapturedLayout::new(
        facts.dwPageSize as usize,
        StartupMapping::control_storage_bytes(),
        native::Lease::storage_bytes(),
    )
}

/// Preflight the same native/map owners in the backend's unchanged192KiB envelope.
#[allow(unsafe_code)]
pub(in crate::windows_lifecycle_pipe) fn backend_layout()
-> Result<CapturedLayout, lifecycle_pipe::LifecyclePipeFailure> {
    let mut facts = windows::Win32::System::SystemInformation::SYSTEM_INFO::default();
    // SAFETY: the OS writes only this fixed local page-size output.
    unsafe { windows::Win32::System::SystemInformation::GetSystemInfo(&mut facts) };
    CapturedLayout::backend(
        facts.dwPageSize as usize,
        StartupMapping::control_storage_bytes(),
        native::Lease::storage_bytes(),
    )
}
