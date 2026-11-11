// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reserve genuine recovery transport and paired Root UI inheritance before carrier launch.
pub(crate) mod inheritance;
pub use crate::recovery_supervisor::*;
pub use crate::windows_lifecycle_pipe::{InheritedLifecycleBootstrap, LifecyclePipeInheritance};
use crate::{
    ExecutionClient,
    admission::RequestKind,
    lifecycle_pipe::{self, Control, Owner, PeerBinding, PeerInput, enrollment::Enrollment},
    scope::lock,
    windows_lifecycle_pipe::{LifecyclePipeAdmissionError, LifecyclePipeBootstrap, native},
};
pub use inheritance::{
    InheritedRecoveryBootstrap, RecoveryCarrierBootstrap, RecoveryCarrierBootstrapRejection,
};
use std::{error::Error, fmt, os::windows::io::OwnedHandle};
use sugarsubstitute_contracts::{
    execution::{AdmissionError, JobClass, JobSpec, Moment, Operation, Scope},
    lifecycle::ProcessKey,
    recovery_supervisor::RecoveryGrantKey,
};

use crate::RegisteredRecoveryGrant;

const METADATA_BYTES: u64 = 65_536;

/// Preserve an actual admission cause or reject a fixed-purpose layout inline.
pub enum RecoveryPipeAdmissionError {
    /// Original central decision or clock conversion cause.
    Transport(LifecyclePipeAdmissionError),
    /// Custody transport uses the fixed existing 192 KiB reservation.
    Reservation {
        /// Submitted total reservation, in bytes.
        requested: u64,
    },
    /// Compiled native/control storage exceeds its original 64 KiB metadata partition.
    Layout {
        /// Actual compiled native/control layout requirement, in bytes.
        required: usize,
    },
}
impl fmt::Debug for RecoveryPipeAdmissionError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        output.write_str("RecoveryPipeAdmissionError { redacted }")
    }
}
impl fmt::Display for RecoveryPipeAdmissionError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        output.write_str("Backend recovery transport admission rejected")
    }
}
impl Error for RecoveryPipeAdmissionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Transport(cause) => Some(cause),
            Self::Reservation { .. } | Self::Layout { .. } => None,
        }
    }
}

/// Pin the immutable recovery bootstrap through native attribute deletion.
pub struct RecoveryBootstrap {
    inner: LifecyclePipeBootstrap,
    key: RecoveryGrantKey,
}
impl RecoveryBootstrap {
    /// Return immutable matching facts, without process or exclusion authority.
    pub const fn key(&self) -> RecoveryGrantKey {
        self.key
    }
    /// Return unchanged original QPC facts for the dedicated invocation.
    pub fn deadline(&self) -> Option<LifecyclePipeDeadline> {
        self.inner.deadline()
    }
    /// Require the actual same-session enrollment allocation and matching key.
    pub fn matches_enrollment(&self, enrollment: &RecoveryEnrollment) -> bool {
        self.key == enrollment.key && enrollment.inner.matches(&self.inner.shared)
    }
    /// Borrow only the already pinned section for the explicit handle allowlist.
    pub fn borrow_inheritance(&self) -> LifecyclePipeInheritance<'_> {
        self.inner.borrow_inheritance()
    }
}

/// Hold the actual suspended-helper enrollment until native adoption or cancellation.
pub struct RecoveryEnrollment {
    inner: Enrollment,
    key: RecoveryGrantKey,
}
/// Preserve original outputs when same-session suspended enrollment rejects.
pub struct RecoveryEnrollmentRejection {
    /// Actual closed transport rejection.
    pub error: LifecyclePipeError,
    /// Original uniquely owned suspended process duplicate.
    pub process: OwnedHandle,
    /// Original retained enrollment, still responsible for cancellation.
    pub enrollment: RecoveryEnrollment,
}
impl RecoveryEnrollment {
    /// Return matching key facts; these do not authenticate a process.
    pub const fn key(&self) -> RecoveryGrantKey {
        self.key
    }
    /// Adopt the exact suspended process before the registry may resume it.
    /// # Errors
    /// Returns the identical process/enrollment owners on mismatch or closure.
    #[expect(
        clippy::result_large_err,
        reason = "Rejection preserves the original OwnedHandle and admitted enrollment inline without allocating"
    )]
    pub fn enroll(
        mut self,
        trusted: RecoveryGrantKey,
        process_key: ProcessKey,
        process: OwnedHandle,
    ) -> Result<(), RecoveryEnrollmentRejection> {
        match self.inner.bind(PeerBinding::Recovery(trusted), process_key, process, |handle| {
            PeerInput::Owned { handle, process: Some(process_key) }
        }) {
            Ok(()) => Ok(()),
            Err((error, process)) => {
                Err(RecoveryEnrollmentRejection { error, process, enrollment: self })
            }
        }
    }
}
impl RecoveryPipeSession {
    /// Transfer the original bootstrap and enrollment together exactly once.
    /// # Errors
    /// Rejects a stopped or already transferred admitted session.
    pub fn take_bootstrap(
        &mut self,
    ) -> Result<Option<(RecoveryBootstrap, RecoveryEnrollment)>, LifecyclePipeError> {
        let Some((inner, enrollment)) = Enrollment::take_bootstrap(&self.control.inner)? else {
            return Ok(None);
        };
        let key = enrollment
            .binding()
            .and_then(PeerBinding::recovery)
            .ok_or(LifecyclePipeError::NotReady)?;
        Ok(Some((RecoveryBootstrap { inner, key }, RecoveryEnrollment { inner: enrollment, key })))
    }
}

/// Reserve the dedicated invocation before taking its inherited section.
pub struct RecoveryClientPermit {
    original_deadline: Moment,
    session: Option<RecoveryPipeSession>,
}
impl RecoveryClientPermit {
    /// Return the one conservative translation of the original handshake.
    pub const fn original_deadline(&self) -> Moment {
        self.original_deadline
    }
    /// Adopt the original inherited section without another admission decision.
    pub fn bind(mut self, section: InheritedRecoveryBootstrap) -> Option<RecoveryPipeSession> {
        let session = self.session.take()?;
        let [section, desktop_section] = section.into_raw();
        session.control.inner.bind_input(PeerInput::RecoveryInherited { section, desktop_section });
        Some(session)
    }
    /// Cancel an unused reservation while retaining its actual typed outcome.
    pub fn cancel_unbound(mut self) -> Option<RecoveryPipeSession> {
        let session = self.session.take()?;
        release_input(&session.control.inner);
        Some(session)
    }
}
impl Drop for RecoveryClientPermit {
    fn drop(&mut self) {
        if let Some(session) = self.session.take() {
            release_input(&session.control.inner);
        }
    }
}
/// Release only the unadopted input pin before normal checked pipe retirement.
fn release_input(control: &Control) {
    lock(&control.shared.state).input_pin = false;
    control.close();
}

impl ExecutionClient {
    /// Admit the fixed recovery server under its unchanged original finite handshake.
    /// # Errors
    /// Rejects nonpersistent scope/spec, wrong class/operation, layout or capacity
    /// before native effects. The original clock/admission cause remains owned.
    pub fn reserve_recovery_pipe_server(
        &self,
        scope: &Scope,
        spec: JobSpec,
        grant: &RegisteredRecoveryGrant,
    ) -> Result<RecoveryPipeSession, RecoveryPipeAdmissionError> {
        self.validate_recovery_pipe(scope, spec)?;
        self.admit_recovery_pipe(scope, spec, Some(grant), grant.original_budget().until(), None)
    }
    /// Reserve the fixed child purpose before adopting an inherited bootstrap.
    /// # Errors
    /// Keeps the inherited input with its caller on scope, counter or admission failure.
    pub fn reserve_recovery_pipe_client(
        &self,
        scope: &Scope,
        spec: JobSpec,
        original: LifecyclePipeDeadline,
    ) -> Result<RecoveryClientPermit, RecoveryPipeAdmissionError> {
        self.validate_recovery_pipe(scope, spec)?;
        let until = self
            .decode_lifecycle_deadline(original)
            .map_err(RecoveryPipeAdmissionError::Transport)?;
        let session = self.admit_recovery_pipe(scope, spec, None, until, Some(original))?;
        Ok(RecoveryClientPermit { original_deadline: until, session: Some(session) })
    }
    /// Preflight this purpose's unchanged transport partitions before construction.
    pub(crate) fn validate_recovery_pipe(
        &self,
        scope: &Scope,
        spec: JobSpec,
    ) -> Result<(), RecoveryPipeAdmissionError> {
        lifecycle_pipe::validate_persistent_scope(&self.core, scope, spec)
            .map_err(RecoveryPipeAdmissionError::Transport)?;
        if spec.class != JobClass::NativeObservation || spec.operation != Operation::NativeLifecycle
        {
            return Err(RecoveryPipeAdmissionError::Transport(
                LifecyclePipeAdmissionError::Admission(AdmissionError::WrongClass {
                    operation: spec.operation,
                    class: spec.class,
                }),
            ));
        }
        if spec.reservation_bytes != MIN_LIFECYCLE_PIPE_BYTES {
            return Err(RecoveryPipeAdmissionError::Reservation {
                requested: spec.reservation_bytes,
            });
        }
        let required = native::Lease::storage_bytes();
        if required as u64 > METADATA_BYTES {
            return Err(RecoveryPipeAdmissionError::Layout { required });
        }
        Ok(())
    }
    /// Claim fixed metadata before allocating the purpose-specific native lease.
    fn admit_recovery_pipe(
        &self,
        scope: &Scope,
        spec: JobSpec,
        grant: Option<&RegisteredRecoveryGrant>,
        handshake: Moment,
        original: Option<LifecyclePipeDeadline>,
    ) -> Result<RecoveryPipeSession, RecoveryPipeAdmissionError> {
        let authority = grant.cloned();
        let key = authority.as_ref().map(RegisteredRecoveryGrant::key);
        let kind = authority.as_ref().map_or(RequestKind::Native, RequestKind::RecoveryPipe);
        let (inner, completion) = lifecycle_pipe::admit_owned(
            &self.core,
            scope,
            spec,
            lifecycle_pipe::PipeAdmission {
                binding: key.map(PeerBinding::Recovery),
                handshake,
                owner: Owner::Recovery,
            },
            kind,
            (),
            |(), context| {
                let metadata = context
                    .memory()
                    .claim(METADATA_BYTES)
                    .map_err(|cause| (AdmissionError::Memory(cause), ()))?;
                let lease = match (key, original) {
                    (Some(key), _) => native::Lease::recovery_server(key),
                    (None, Some(original)) => native::Lease::recovery_client(original),
                    (None, None) => return Err((AdmissionError::DeadlineRequired, ())),
                };
                if let Some(grant) = &authority {
                    let mut stages = lock(&grant.state().map_err(|cause| (cause, ()))?.stages);
                    if stages.pipe.is_some() || stages.closed {
                        return Err((AdmissionError::InvalidCaptureAdmission, ()));
                    }
                    stages.pipe = Some(spec.id);
                }
                Ok(lifecycle_pipe::PreparedPipeLease {
                    lease: Box::new(lease),
                    metadata: Some(metadata),
                })
            },
        )
        .map_err(|(cause, ())| {
            RecoveryPipeAdmissionError::Transport(LifecyclePipeAdmissionError::Admission(cause))
        })?;
        Ok(RecoveryPipeSession { control: RecoveryPipeControl { inner }, completion })
    }
}
