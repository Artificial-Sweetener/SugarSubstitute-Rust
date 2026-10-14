// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reserve backend custody transport before native adoption or helper launch.
pub use crate::backend_custody::*;
pub use crate::windows_lifecycle_pipe::{InheritedLifecycleBootstrap, LifecyclePipeInheritance};
use crate::{
    ExecutionClient,
    admission::RequestKind,
    lifecycle_pipe::{self, Control, Owner, PeerBinding, PeerInput, enrollment::Enrollment},
    scope::lock,
    windows_lifecycle_pipe::{LifecyclePipeAdmissionError, LifecyclePipeBootstrap, native},
};
use std::{error::Error, fmt, os::windows::io::OwnedHandle};
use sugarsubstitute_contracts::{
    backend_custody::BackendCustodyKey,
    execution::{AdmissionError, JobClass, JobSpec, Moment, Operation, Scope},
    lifecycle::ProcessKey,
};

pub use crate::windows_lifecycle_pipe::captured::backend::CapturedBackendCustodyStart;

const METADATA_BYTES: u64 = 65_536;

/// Preserve an actual admission cause or reject a fixed-purpose layout inline.
pub enum BackendCustodyPipeAdmissionError {
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
impl fmt::Debug for BackendCustodyPipeAdmissionError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        output.write_str("BackendCustodyPipeAdmissionError { redacted }")
    }
}
impl fmt::Display for BackendCustodyPipeAdmissionError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        output.write_str("Backend custody transport admission rejected")
    }
}
impl Error for BackendCustodyPipeAdmissionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Transport(cause) => Some(cause),
            Self::Reservation { .. } | Self::Layout { .. } => None,
        }
    }
}

/// Pin the immutable custody bootstrap through native attribute deletion.
pub struct BackendCustodyBootstrap {
    inner: LifecyclePipeBootstrap,
    key: BackendCustodyKey,
}
impl BackendCustodyBootstrap {
    /// Return immutable matching facts, without process or exclusion authority.
    pub const fn key(&self) -> BackendCustodyKey {
        self.key
    }
    /// Match the genuine purpose-specific captured server marker to this exact key.
    /// Pair with matches_enrollment; these cached facts alone grant no native authority.
    pub fn captures(&self, key: BackendCustodyKey) -> bool {
        self.key == key && self.inner.shared.captured
    }
    /// Return unchanged original QPC facts for the dedicated invocation.
    pub fn deadline(&self) -> Option<LifecyclePipeDeadline> {
        self.inner.deadline()
    }
    /// Require the actual same-session enrollment allocation and matching key.
    pub fn matches_enrollment(&self, enrollment: &BackendCustodyEnrollment) -> bool {
        self.key == enrollment.key && enrollment.inner.matches(&self.inner.shared)
    }
    /// Borrow only the already pinned section for the explicit handle allowlist.
    pub fn borrow_inheritance(&self) -> LifecyclePipeInheritance<'_> {
        self.inner.borrow_inheritance()
    }
}

/// Hold the actual suspended-helper enrollment until native adoption or cancellation.
pub struct BackendCustodyEnrollment {
    inner: Enrollment,
    key: BackendCustodyKey,
}
/// Preserve original outputs when same-session suspended enrollment rejects.
pub struct BackendCustodyEnrollmentRejection {
    /// Actual closed transport rejection.
    pub error: LifecyclePipeError,
    /// Original uniquely owned suspended process duplicate.
    pub process: OwnedHandle,
    /// Original retained enrollment, still responsible for cancellation.
    pub enrollment: BackendCustodyEnrollment,
}
impl BackendCustodyEnrollment {
    /// Return matching key facts; these do not authenticate a process.
    pub const fn key(&self) -> BackendCustodyKey {
        self.key
    }
    /// Adopt the exact suspended process before the registry may resume it.
    /// # Errors
    /// Returns the identical process/enrollment owners on mismatch or closure.
    pub fn enroll(
        mut self,
        trusted: BackendCustodyKey,
        process_key: ProcessKey,
        process: OwnedHandle,
    ) -> Result<(), BackendCustodyEnrollmentRejection> {
        match self.inner.bind(
            PeerBinding::BackendCustody(trusted),
            process_key,
            process,
            |handle| PeerInput::Owned { handle, process: Some(process_key) },
        ) {
            Ok(()) => Ok(()),
            Err((error, process)) => {
                Err(BackendCustodyEnrollmentRejection { error, process, enrollment: self })
            }
        }
    }
}
impl BackendCustodyPipeSession {
    /// Transfer the original bootstrap and enrollment together exactly once.
    /// # Errors
    /// Rejects a stopped or already transferred admitted session.
    pub fn take_bootstrap(
        &mut self,
    ) -> Result<Option<(BackendCustodyBootstrap, BackendCustodyEnrollment)>, LifecyclePipeError>
    {
        let Some((inner, enrollment)) = Enrollment::take_bootstrap(&self.control.inner)? else {
            return Ok(None);
        };
        let key = enrollment
            .binding()
            .and_then(PeerBinding::backend_custody)
            .ok_or(LifecyclePipeError::NotReady)?;
        Ok(Some((
            BackendCustodyBootstrap { inner, key },
            BackendCustodyEnrollment { inner: enrollment, key },
        )))
    }
}

/// Reserve the dedicated invocation before taking its inherited section.
pub struct BackendCustodyClientPermit {
    original_deadline: Moment,
    session: Option<BackendCustodyPipeSession>,
}
impl BackendCustodyClientPermit {
    /// Compose only an already-admitted same-owner captured purpose without readmission.
    pub(crate) fn from_captured(
        original_deadline: Moment,
        session: BackendCustodyPipeSession,
    ) -> Self {
        Self { original_deadline, session: Some(session) }
    }

    /// Return the one conservative translation of the original handshake.
    pub const fn original_deadline(&self) -> Moment {
        self.original_deadline
    }
    /// Adopt the original inherited section without another admission decision.
    pub fn bind(
        mut self,
        section: InheritedLifecycleBootstrap,
    ) -> Option<BackendCustodyPipeSession> {
        let session = self.session.take()?;
        session.control.inner.bind_input(PeerInput::Inherited { section: section.into_raw() });
        Some(session)
    }
    /// Cancel an unused reservation while retaining its actual typed outcome.
    pub fn cancel_unbound(mut self) -> Option<BackendCustodyPipeSession> {
        let session = self.session.take()?;
        release_input(&session.control.inner);
        Some(session)
    }
}
impl Drop for BackendCustodyClientPermit {
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
    /// Admit the fixed custody server under its unchanged original finite handshake.
    /// # Errors
    /// Rejects nonpersistent scope/spec, wrong class/operation, layout or capacity
    /// before native effects. The original clock/admission cause remains owned.
    pub fn reserve_backend_custody_pipe_server(
        &self,
        scope: &Scope,
        spec: JobSpec,
        key: BackendCustodyKey,
        handshake: Moment,
    ) -> Result<BackendCustodyPipeSession, BackendCustodyPipeAdmissionError> {
        self.validate_backend_custody_pipe(scope, spec)?;
        self.admit_backend_custody_pipe(scope, spec, Some(key), handshake, None)
    }
    /// Reserve the fixed child purpose before adopting an inherited bootstrap.
    /// # Errors
    /// Keeps the inherited input with its caller on scope, counter or admission failure.
    pub fn reserve_backend_custody_pipe_client(
        &self,
        scope: &Scope,
        spec: JobSpec,
        original: LifecyclePipeDeadline,
    ) -> Result<BackendCustodyClientPermit, BackendCustodyPipeAdmissionError> {
        self.validate_backend_custody_pipe(scope, spec)?;
        let until = self
            .decode_lifecycle_deadline(original)
            .map_err(BackendCustodyPipeAdmissionError::Transport)?;
        let session = self.admit_backend_custody_pipe(scope, spec, None, until, Some(original))?;
        Ok(BackendCustodyClientPermit { original_deadline: until, session: Some(session) })
    }
    /// Preflight this purpose's unchanged transport partitions before construction.
    pub(crate) fn validate_backend_custody_pipe(
        &self,
        scope: &Scope,
        spec: JobSpec,
    ) -> Result<(), BackendCustodyPipeAdmissionError> {
        lifecycle_pipe::validate_persistent_scope(&self.core, scope, spec)
            .map_err(BackendCustodyPipeAdmissionError::Transport)?;
        if spec.class != JobClass::NativeObservation || spec.operation != Operation::NativeLifecycle
        {
            return Err(BackendCustodyPipeAdmissionError::Transport(
                LifecyclePipeAdmissionError::Admission(AdmissionError::WrongClass {
                    operation: spec.operation,
                    class: spec.class,
                }),
            ));
        }
        if spec.reservation_bytes != MIN_LIFECYCLE_PIPE_BYTES {
            return Err(BackendCustodyPipeAdmissionError::Reservation {
                requested: spec.reservation_bytes,
            });
        }
        let required = native::Lease::storage_bytes();
        if required as u64 > METADATA_BYTES {
            return Err(BackendCustodyPipeAdmissionError::Layout { required });
        }
        Ok(())
    }
    /// Claim fixed metadata before allocating the purpose-specific native lease.
    fn admit_backend_custody_pipe(
        &self,
        scope: &Scope,
        spec: JobSpec,
        key: Option<BackendCustodyKey>,
        handshake: Moment,
        original: Option<LifecyclePipeDeadline>,
    ) -> Result<BackendCustodyPipeSession, BackendCustodyPipeAdmissionError> {
        let (inner, completion) = lifecycle_pipe::admit_owned(
            &self.core,
            scope,
            spec,
            lifecycle_pipe::PipeAdmission {
                binding: key.map(PeerBinding::BackendCustody),
                handshake,
                owner: Owner::BackendCustody,
            },
            RequestKind::Native,
            (),
            |(), context| {
                let metadata = context
                    .memory()
                    .claim(METADATA_BYTES)
                    .map_err(|cause| (AdmissionError::Memory(cause), ()))?;
                let lease = match (key, original) {
                    (Some(key), _) => native::Lease::backend_custody_server(key),
                    (None, Some(original)) => native::Lease::backend_custody_client(original),
                    (None, None) => return Err((AdmissionError::DeadlineRequired, ())),
                };
                Ok(lifecycle_pipe::PreparedPipeLease {
                    lease: Box::new(lease),
                    metadata: Some(metadata),
                })
            },
        )
        .map_err(|(cause, ())| {
            BackendCustodyPipeAdmissionError::Transport(LifecyclePipeAdmissionError::Admission(
                cause,
            ))
        })?;
        Ok(BackendCustodyPipeSession { control: BackendCustodyPipeControl { inner }, completion })
    }
}
