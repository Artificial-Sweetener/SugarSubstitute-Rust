// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Transport native lifecycle messages through admitted local authenticated sessions.
//! Codec, replay, resource policy and native UI-loop proof remain with their owners.
pub(crate) mod captured;
#[allow(unsafe_code)]
mod startup_mapping;
pub(crate) use startup_mapping::{StartupViewLoan, StartupViewRetention};
mod captured_layout;
mod recovery_pair;
pub use crate::lifecycle_pipe::MIN_CAPTURED_LIFECYCLE_PIPE_BYTES;
pub use crate::lifecycle_pipe::{
    AuthenticatedStartupSnapshot, LifecycleClientCapability, LifecyclePipeBootstrap,
    LifecyclePipeControl, LifecyclePipeDeadline, LifecyclePipeError, LifecyclePipeFailure,
    LifecyclePipeObservation, LifecyclePipeOperationId, LifecyclePipeReport,
    LifecyclePipeRetirement, LifecyclePipeSession, LifecyclePipeWriteBuffer,
    LifecyclePipeWriteRejection, MAX_LIFECYCLE_MESSAGE_BYTES, MIN_LIFECYCLE_PIPE_BYTES,
    NativeWriteAdmission, NativeWriteRevocation, ReceivedLifecycleFrame, VerifiedLifecyclePeer,
};
use crate::{
    ExecutionClient,
    lifecycle_pipe::{self, PeerInput},
    native::NativeObservationError,
    scope::lock,
};
pub use captured::child::CapturedStartup;
pub use captured::parent::{CapturedLifecycleStart, RecoveryDesktopBootstrap};
use std::{
    fmt,
    os::windows::io::{BorrowedHandle, OwnedHandle},
};
use sugarsubstitute_contracts::{
    execution::{AdmissionError, JobSpec, Moment, Scope},
    lifecycle::{ProcessKey, RunKey},
};

#[allow(unsafe_code)]
#[path = "windows_lifecycle_pipe/native.rs"]
pub(crate) mod native;
/// Preserve clock/admission rejection without taking ownership of inherited input.
pub enum LifecyclePipeAdmissionError {
    /// Actual central execution admission rejection.
    Admission(AdmissionError),
    /// Original bounded native counter/conversion cause.
    Clock(NativeObservationError),
    /// A client specification already supplied another independently renewable deadline.
    ConflictingDeadline,
}
impl fmt::Debug for LifecyclePipeAdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("LifecyclePipeAdmissionError { redacted }")
    }
}
impl fmt::Display for LifecyclePipeAdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Lifecycle pipe admission rejected")
    }
}
impl std::error::Error for LifecyclePipeAdmissionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Admission(e) => Some(e),
            Self::Clock(e) => Some(e),
            Self::ConflictingDeadline => None,
        }
    }
}
impl LifecyclePipeFailure {
    /// Return the original numeric Win32 cause when it is an HRESULT_FROM_WIN32.
    /// This pure query preserves the source chain and excludes panic/invariant causes.
    /// Callers must also match the original native stage and fatal classifications.
    pub fn win32_code(&self) -> Option<u32> {
        use std::error::Error;
        let cause = self.native_cause().source()?.downcast_ref::<windows::core::Error>()?;
        let code = cause.code().0 as u32;
        (code & 0xffff_0000 == 0x8007_0000 && code & 0xffff != 0).then_some(code & 0xffff)
    }
}
/// One bootstrap token exclusively inherited by a dedicated child invocation.
/// Construction performs no native operation and Drop closes nothing. Failed
/// reservation requires that invocation to fail/exit; its parent observes actual
/// process/family settlement without claiming clean child-side handle retirement.
pub struct InheritedLifecycleBootstrap {
    raw: usize,
}
impl InheritedLifecycleBootstrap {
    pub(crate) fn into_raw(self) -> usize {
        self.raw
    }

    /// Validate a positive pointer-sized non-pseudo token without taking a handle.
    /// Only supply the dedicated child's exclusively inherited section token.
    pub fn new(value: u64) -> Result<Self, LifecyclePipeError> {
        let raw = usize::try_from(value).map_err(|_| LifecyclePipeError::InvalidMessage)?;
        if raw == 0 || raw >= usize::MAX - 15 {
            return Err(LifecyclePipeError::InvalidMessage);
        }
        Ok(Self { raw })
    }
}
/// Reserve child lifetime before transferring its sole inherited bootstrap handle.
/// Cancellation/timeout retains this input pin until bind or Drop; no late native
/// input can arrive after terminal. Failed isolated admission leaves input with
/// its caller, whose actual process death must be observed by the parent.
pub struct LifecyclePipeClientPermit {
    original_deadline: Moment,
    session: Option<LifecyclePipeSession>,
}
impl LifecyclePipeClientPermit {
    /// Return the conservatively translated immutable deadline in this executor's epoch.
    /// Use this exact fact for the child's initial Hello/read/write commands.
    pub const fn original_deadline(&self) -> Moment {
        self.original_deadline
    }

    /// Transfer exclusive inherited section ownership without another capacity decision.
    /// The worker validates, clears inheritance, maps/reads and checks actual closure.
    pub fn bind(mut self, section: InheritedLifecycleBootstrap) -> Option<LifecyclePipeSession> {
        let session = self.session.take()?;
        session.control.bind_input(PeerInput::Inherited { section: section.into_raw() });
        Some(session)
    }
    /// Cancel an unused reservation while retaining actual typed terminal delivery.
    pub fn cancel_unbound(mut self) -> Option<LifecyclePipeSession> {
        let session = self.session.take()?;
        {
            lock(&session.control.inner.shared.state).input_pin = false;
        }
        session.control.close();
        Some(session)
    }
}
impl Drop for LifecyclePipeClientPermit {
    fn drop(&mut self) {
        if let Some(session) = self.session.take() {
            {
                lock(&session.control.inner.shared.state).input_pin = false;
            }
            session.control.close();
        }
    }
}
impl ExecutionClient {
    /// Reserve a v3 child transport before transferring its sole inherited section.
    /// The actual native worker authenticates the full peer and two process capabilities;
    /// decoded matching facts cannot produce startup or capture readiness.
    pub fn reserve_captured_lifecycle_pipe_client(
        &self,
        scope: &Scope,
        spec: JobSpec,
        original: LifecyclePipeDeadline,
    ) -> Result<LifecyclePipeClientPermit, LifecyclePipeAdmissionError> {
        use sugarsubstitute_contracts::execution::{JobClass, Operation};
        if spec.deadline.is_some() {
            return Err(LifecyclePipeAdmissionError::ConflictingDeadline);
        }
        if spec.class != JobClass::NativeObservation
            || spec.operation != Operation::NativeLifecycle
            || spec.reservation_bytes != MIN_CAPTURED_LIFECYCLE_PIPE_BYTES
            || spec.replacement.is_some()
        {
            return Err(LifecyclePipeAdmissionError::Admission(
                AdmissionError::InvalidCaptureAdmission,
            ));
        }
        let until = self.decode_lifecycle_deadline(original)?;
        let layout = captured::parent::layout()
            .map_err(|cause| LifecyclePipeAdmissionError::Clock(cause.native_cause().clone()))?;
        let (inner, completion) = lifecycle_pipe::admit_owned(
            &self.core,
            scope,
            spec,
            lifecycle_pipe::PipeAdmission {
                binding: None,
                handshake: until,
                owner: lifecycle_pipe::Owner::Desktop,
            },
            crate::admission::RequestKind::Native,
            (),
            |(), context| {
                let native_claim =
                    context.memory().claim(layout.native_bytes as u64).map_err(|cause| {
                        (AdmissionError::Unexpected(crate::runtime::error(cause)), ())
                    })?;
                let map_claim =
                    context.memory().claim(layout.map_bytes as u64).map_err(|cause| {
                        (AdmissionError::Unexpected(crate::runtime::error(cause)), ())
                    })?;
                let mut lease = native::Lease::client(original);
                lease.captured_client = true;
                lease
                    .install_captured_mapping(
                        map_claim,
                        crate::native::Signal::new(&self.core),
                        layout.map_extent,
                    )
                    .map_err(|cause| {
                        (AdmissionError::Unexpected(crate::runtime::error(cause)), ())
                    })?;
                Ok(lifecycle_pipe::PreparedPipeLease {
                    lease: Box::new(lease),
                    metadata: Some(native_claim),
                })
            },
        )
        .map_err(|(cause, ())| LifecyclePipeAdmissionError::Admission(cause))?;
        let session = LifecyclePipeSession { control: LifecyclePipeControl { inner }, completion };
        Ok(LifecyclePipeClientPermit { original_deadline: until, session: Some(session) })
    }
    /// Admit one server before creating a pipe, bootstrap section or native wait.
    /// `run` is a trusted composing-owner fact, not installation/authentication proof.
    /// The native scope should have no deadline so cleanup survives policy expiry;
    /// `original_handshake` is published to the sole private timer owner immediately.
    pub fn reserve_lifecycle_pipe_server(
        &self,
        scope: &Scope,
        spec: JobSpec,
        run: RunKey,
        original_handshake: Moment,
    ) -> Result<LifecyclePipeSession, AdmissionError> {
        lifecycle_pipe::reserve(
            &self.core,
            scope,
            spec,
            Some(run),
            original_handshake,
            Box::new(native::Lease::server(run)),
        )
    }
    /// Translate the unchanged parent deadline before admitting any inherited handle.
    /// Sample local Moment BEFORE QPC, require exact frequency and use checked floor
    /// arithmetic. `spec.deadline` must be None; the fixed handshake deadline is
    /// published before admission. After authentication the persistent session
    /// remains alive for separately bounded commands; the worker never changes a JobSpec.
    pub fn reserve_lifecycle_pipe_client(
        &self,
        scope: &Scope,
        spec: JobSpec,
        original: LifecyclePipeDeadline,
    ) -> Result<LifecyclePipeClientPermit, LifecyclePipeAdmissionError> {
        if spec.deadline.is_some() {
            return Err(LifecyclePipeAdmissionError::ConflictingDeadline);
        }
        let until = self.decode_lifecycle_deadline(original)?;
        let session = lifecycle_pipe::reserve(
            &self.core,
            scope,
            spec,
            None,
            until,
            Box::new(native::Lease::client(original)),
        )
        .map_err(LifecyclePipeAdmissionError::Admission)?;
        Ok(LifecyclePipeClientPermit { original_deadline: until, session: Some(session) })
    }
}
/// Transfer one trusted suspended-desktop identity to the execution owner.
/// The registry retains this non-cloneable pin from launch admission until enrollment.
/// Drop cancels the session and releases only the pin; it performs no native closure.
pub struct LifecycleDesktopEnrollment {
    inner: lifecycle_pipe::enrollment::Enrollment,
    run: RunKey,
}
/// Return both untransferred authorities when exact enrollment is rejected.
pub struct LifecycleDesktopEnrollmentRejection {
    /// Typed redacted rejection.
    pub error: LifecyclePipeError,
    /// Original process duplicate, still owned by its trusted native registry.
    pub process: OwnedHandle,
    /// Original launch pin, available for explicit cancellation or a valid transfer.
    pub enrollment: LifecycleDesktopEnrollment,
}
impl LifecycleDesktopEnrollment {
    /// Original admitted run; identity facts do not prove caller authentication.
    pub fn run(&self) -> RunKey {
        self.run
    }
    /// Transfer the exact suspended process before the registry selects ResumeThread.
    /// `trusted_run` and `key` come from the registry, never decoded request fields.
    /// Rejection preserves the process duplicate and pin, including after cancellation.
    pub fn enroll(
        mut self,
        trusted_run: RunKey,
        key: ProcessKey,
        process: OwnedHandle,
    ) -> Result<(), LifecycleDesktopEnrollmentRejection> {
        match self.inner.bind(
            lifecycle_pipe::PeerBinding::Desktop(trusted_run),
            key,
            process,
            |handle| PeerInput::Owned { handle, process: Some(key) },
        ) {
            Ok(()) => Ok(()),
            Err((error, process)) => {
                Err(LifecycleDesktopEnrollmentRejection { error, process, enrollment: self })
            }
        }
    }
}
impl LifecyclePipeControl {
    /// Take the single process-enrollment pin before admitting its launch.
    /// A registry holds it through suspended creation and binds before resuming.
    pub fn take_desktop_enrollment(
        &self,
    ) -> Result<LifecycleDesktopEnrollment, LifecyclePipeError> {
        lifecycle_pipe::enrollment::Enrollment::take(&self.inner).and_then(|inner| {
            let run = inner
                .binding()
                .and_then(lifecycle_pipe::PeerBinding::desktop)
                .ok_or(LifecyclePipeError::NotReady)?;
            Ok(LifecycleDesktopEnrollment { inner, run })
        })
    }
}
/// Borrow only the read-only inherited bootstrap section required by HANDLE_LIST.
pub struct LifecyclePipeInheritance<'a> {
    /// A fixed immutable section; no command/environment capability bytes are exposed.
    pub section: BorrowedHandle<'a>,
    /// Actual retained creator/handler pins for captured startup, covered by the same launch pin.
    pub capture_processes: Option<[BorrowedHandle<'a>; 2]>,
}
impl LifecyclePipeBootstrap {
    /// Return the exact original admitted run without granting authentication.
    pub fn run(&self) -> Option<RunKey> {
        lock(&self.shared.state).binding.and_then(lifecycle_pipe::PeerBinding::desktop)
    }
    /// Verify both launch artifacts belong to the same admitted session and run.
    /// Matching names/IDs alone cannot replace this single-owner provenance check.
    pub fn matches_enrollment(&self, enrollment: &LifecycleDesktopEnrollment) -> bool {
        enrollment.inner.matches(&self.shared) && self.run() == Some(enrollment.run())
    }
    /// Borrow a pinned read-only section through CreateProcess and attribute deletion.
    #[allow(unsafe_code)]
    pub fn borrow_inheritance(&self) -> LifecyclePipeInheritance<'_> {
        // SAFETY: the non-cloneable launch pin prevents checked section closure.
        let capture_processes = lock(&self.shared.state).capture_inherited;
        LifecyclePipeInheritance {
            // SAFETY: the same exclusive bootstrap pin and retained parent source prevent closure.
            capture_processes: capture_processes.map(|handles| {
                handles.map(|raw| {
                    // SAFETY: the live bootstrap/source pin retains this inherited handle throughout the borrow.
                    unsafe { BorrowedHandle::borrow_raw(raw as *mut _) }
                })
            }),
            // SAFETY: the live non-cloneable pin prevents native closure throughout this borrow.
            section: unsafe { BorrowedHandle::borrow_raw(self.handle() as *mut _) },
        }
    }
}

#[cfg(test)]
#[path = "windows_lifecycle_pipe/input_tests.rs"]
mod input_tests;

#[cfg(test)]
#[allow(unsafe_code)]
#[path = "windows_lifecycle_pipe/native_abi_tests.rs"]
mod native_abi_tests;
