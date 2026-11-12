// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit the genuine backend capture pin before its sole immutable bootstrap is prepared.
use crate::{
    ExecutionClient,
    admission::RequestKind,
    backend_custody::{BackendCustodyPipeControl, BackendCustodyPipeSession},
    lifecycle_pipe::{self, Owner, PeerBinding},
    scope::lock,
    windows_backend_custody::{BackendCustodyClientPermit, BackendCustodyPipeAdmissionError},
    windows_lifecycle_pipe::{LifecyclePipeAdmissionError, LifecyclePipeDeadline},
};
use crate::{
    capture_attach::CaptureAttachLaunchPin,
    lifecycle_pipe::{PipeAdmission, PreparedPipeLease, captured_admission::AdmissionBinding},
    native::Signal,
    windows_lifecycle_pipe::{
        captured::parent::{ParentSource, backend_layout},
        native,
    },
};
use sugarsubstitute_contracts::crash::CaptureTarget;
use sugarsubstitute_contracts::{
    backend_custody::BackendCustodyKey,
    execution::{AdmissionError, JobSpec, Moment, Scope},
};
/// Move-only original backend capture pin, retained unchanged through admission rejection.
pub struct CapturedBackendCustodyStart {
    pin: Option<CaptureAttachLaunchPin>,
}
impl CapturedBackendCustodyStart {
    /// Borrow-check the actual target before transferring the original launch pin.
    /// Performs no allocation, native operation, new admission or deadline observation.
    /// # Errors
    /// Foreign/absent pins leave the caller's original Option unchanged.
    pub fn new(
        key: BackendCustodyKey,
        pin: &mut Option<CaptureAttachLaunchPin>,
    ) -> Result<Self, AdmissionError> {
        if pin.as_ref().is_none_or(|pin| pin.target() != CaptureTarget::BackendCustody(key)) {
            return Err(AdmissionError::InvalidCaptureAdmission);
        }
        Ok(Self { pin: pin.take() })
    }
    /// Observe consumption of this exact original pin without claiming native readiness.
    pub fn is_consumed(&self) -> bool {
        self.pin.is_none()
    }
    /// Release the identical unconsumed pin and retain its original checked retirement delivery.
    /// The existing attachment owner latches cleanup once; this samples no replacement clock
    /// and performs no native operation. A consumed start has no pin to release.
    pub fn release(&mut self) -> Option<crate::capture_attach::CaptureAttachRetirement> {
        self.pin.take().map(CaptureAttachLaunchPin::release)
    }
}
impl ExecutionClient {
    /// Admit the backend's sole captured v5 bootstrap through original pin/clock fences.
    /// The existing192KiB transport and parent16KiB pin claims remain unchanged.
    /// # Errors
    /// Rejection preserves this original start and all captured source/native owners.
    pub fn reserve_captured_backend_custody_pipe_server(
        &self,
        scope: &Scope,
        spec: JobSpec,
        key: BackendCustodyKey,
        original_handshake: Moment,
        start: &mut CapturedBackendCustodyStart,
    ) -> Result<BackendCustodyPipeSession, AdmissionError> {
        self.validate_backend_custody_pipe(scope, spec).map_err(|cause| match cause {
            BackendCustodyPipeAdmissionError::Transport(
                LifecyclePipeAdmissionError::Admission(cause),
            ) => cause,
            BackendCustodyPipeAdmissionError::Reservation { .. } => {
                AdmissionError::InvalidCaptureAdmission
            }
            cause => AdmissionError::Unexpected(crate::runtime::error(cause)),
        })?;
        let pin = start.pin.as_ref().ok_or(AdmissionError::InvalidCaptureAdmission)?;
        let binding = AdmissionBinding::backend(key, original_handshake, pin);
        let layout = backend_layout()
            .map_err(|cause| AdmissionError::Unexpected(crate::runtime::error(cause)))?;
        let factory_binding = binding.clone();
        let (inner, completion) = lifecycle_pipe::admit_owned(
            &self.core,
            scope,
            spec,
            PipeAdmission {
                binding: Some(PeerBinding::BackendCustody(key)),
                handshake: original_handshake,
                owner: Owner::BackendCustody,
            },
            RequestKind::CapturedLifecycle(&binding),
            start,
            |start, context| {
                let native_claim = match context.memory().claim(layout.native_bytes as u64) {
                    Ok(claim) => claim,
                    Err(cause) => return Err((AdmissionError::Memory(cause), start)),
                };
                let map_claim = match context.memory().claim(layout.map_bytes as u64) {
                    Ok(claim) => claim,
                    Err(cause) => return Err((AdmissionError::Memory(cause), start)),
                };
                let (endpoint, processes) = match start.pin.as_ref().and_then(|pin| {
                    let state = lock(&pin.shared.state);
                    Some((state.endpoint?, state.handles?))
                }) {
                    Some(facts) => facts,
                    None => return Err((AdmissionError::InvalidCaptureAdmission, start)),
                };
                let mut lease = native::Lease::backend_custody_server(key);
                if let Err(cause) = lease.install_captured_mapping(
                    map_claim,
                    Signal::new(&self.core),
                    layout.map_extent,
                ) {
                    return Err((AdmissionError::Unexpected(crate::runtime::error(cause)), start));
                }
                let Some(pin) = start.pin.take() else {
                    return Err((AdmissionError::InvalidCaptureAdmission, start));
                };
                lease.captured_parent = Some(ParentSource {
                    pin,
                    startup: None,
                    binding: factory_binding,
                    endpoint,
                    processes,
                });
                Ok(PreparedPipeLease { lease: Box::new(lease), metadata: Some(native_claim) })
            },
        )
        .map_err(|(cause, _start)| cause)?;
        Ok(BackendCustodyPipeSession { control: BackendCustodyPipeControl { inner }, completion })
    }
    /// Reserve the captured backend purpose before adopting its actual inherited section.
    /// Actual native self/parent/handler/capability checks precede publishing the input.
    /// # Errors
    /// Rejects wrong fixed spec, layout, scope/counter/capacity failures without taking input.
    pub fn reserve_captured_backend_custody_pipe_client(
        &self,
        scope: &Scope,
        spec: JobSpec,
        original: LifecyclePipeDeadline,
    ) -> Result<BackendCustodyClientPermit, BackendCustodyPipeAdmissionError> {
        self.validate_backend_custody_pipe(scope, spec)?;
        let until = self
            .decode_lifecycle_deadline(original)
            .map_err(BackendCustodyPipeAdmissionError::Transport)?;
        let layout = backend_layout().map_err(|cause| {
            BackendCustodyPipeAdmissionError::Transport(LifecyclePipeAdmissionError::Clock(
                cause.native_cause().clone(),
            ))
        })?;
        let (inner, completion) = lifecycle_pipe::admit_owned(
            &self.core,
            scope,
            spec,
            PipeAdmission { binding: None, handshake: until, owner: Owner::BackendCustody },
            RequestKind::Native,
            (),
            |(), context| {
                let native_claim = context
                    .memory()
                    .claim(layout.native_bytes as u64)
                    .map_err(|cause| (AdmissionError::Memory(cause), ()))?;
                let map_claim = context
                    .memory()
                    .claim(layout.map_bytes as u64)
                    .map_err(|cause| (AdmissionError::Memory(cause), ()))?;
                let mut lease = native::Lease::backend_custody_client(original);
                lease.captured_client = true;
                lease
                    .install_captured_mapping(map_claim, Signal::new(&self.core), layout.map_extent)
                    .map_err(|cause| {
                        (AdmissionError::Unexpected(crate::runtime::error(cause)), ())
                    })?;
                Ok(PreparedPipeLease { lease: Box::new(lease), metadata: Some(native_claim) })
            },
        )
        .map_err(|(cause, ())| {
            BackendCustodyPipeAdmissionError::Transport(LifecyclePipeAdmissionError::Admission(
                cause,
            ))
        })?;
        let session =
            BackendCustodyPipeSession { control: BackendCustodyPipeControl { inner }, completion };
        Ok(BackendCustodyClientPermit::from_captured(until, session))
    }
}
impl BackendCustodyPipeControl {
    /// Transfer the same actual authenticated backend capture input once before source bind.
    /// A historical prepared key, decoded bytes or stopped session cannot produce this owner.
    pub fn take_captured_input(&self) -> Option<crate::InheritedCaptureAttachInput> {
        let mut state = lock(&self.inner.shared.state);
        if state.stopped || !state.authenticated {
            return None;
        }
        let target = state.binding?.capture_target()?;
        match state.captured_startup.take()? {
            crate::windows_lifecycle_pipe::captured::child::CapturedInput::BackendCustody(
                input,
            ) if input.target() == target => Some(input),
            other => {
                state.captured_startup = Some(other);
                None
            }
        }
    }
}
