// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Move complete inherited inputs into actual bounded native work or return them unchanged.
use super::{
    NativeCaptureDriver,
    delivery::Delivery,
    input::InheritedCaptureAttachInput,
    metadata::DesktopCaptureCustodyRef,
    process_lifetime::ProcessLifetimeCapture,
    types::DesktopCaptureAttached,
    work::{DesktopCaptureWork, WorkInput},
};
use crate::{
    admission::{Core, RequestKind},
    capture_attach::{
        control::{CaptureCleanupSnapshot, CleanupTrigger},
        types::CaptureAttachFailure,
    },
    task::{JobHandle, QueuedJob, Stop, Work},
};
use std::{fmt, sync::Arc};
use sugarsubstitute_contracts::execution::{
    AdmissionError, JobClass, JobSpec, Operation, OwnedJob, Scope,
};

/// Retain the actual complete rejected native input/driver; no effect has started.
pub struct DesktopCaptureAdmissionRejection {
    /// Actual admission/preallocation cause, with original accounting identity.
    pub error: AdmissionError,
    /// Identical untransferred authenticated input and its real native lease.
    pub input: InheritedCaptureAttachInput,
    /// Identical inert driver; no partial native selection was invoked by admission.
    pub driver: Box<dyn NativeCaptureDriver>,
}
impl DesktopCaptureAdmissionRejection {
    /// Consume the genuine cause and original move-only owners without replacing them.
    pub fn into_parts(
        self,
    ) -> (AdmissionError, InheritedCaptureAttachInput, Box<dyn NativeCaptureDriver>) {
        (self.error, self.input, self.driver)
    }
}
impl fmt::Debug for DesktopCaptureAdmissionRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DesktopCaptureAdmissionRejection")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}
impl fmt::Display for DesktopCaptureAdmissionRejection {
    /// Format only the genuine admission cause; retained native inputs stay redacted.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(f)
    }
}
impl std::error::Error for DesktopCaptureAdmissionRejection {
    /// Preserve the authoritative admission cause without replacing its complete input.
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}
/// Observe original cleanup and consume actual permanent transfer separately from Ready.
/// No native handle, executor primitive or new deadline is exposed.
pub struct DesktopCaptureControl {
    shared: DesktopCaptureCustodyRef,
    delivery: Delivery,
}
impl DesktopCaptureControl {
    /// Invalidate fresh effects and claim the same original ten/fifteen-second cleanup.
    pub fn request_cleanup(&self) -> CaptureCleanupSnapshot {
        self.shared.request(Stop::Cancelled, CleanupTrigger::Released)
    }
    /// Read cache-only original transaction facts without observing a provider.
    pub fn cleanup_snapshot(&self) -> CaptureCleanupSnapshot {
        self.shared.cleanup.snapshot()
    }
    /// Consume actual temporary-settlement/permanent-retention handoff once.
    /// Failure never rewrites the original registered job outcome.
    pub fn try_take_process_lifetime(
        &self,
    ) -> Option<Result<ProcessLifetimeCapture, CaptureAttachFailure>> {
        self.delivery.take()
    }
}
/// One actual terminal job plus independent retained-custody observation.
pub struct DesktopCaptureSession {
    job: OwnedJob<DesktopCaptureAttached, CaptureAttachFailure>,
    control: DesktopCaptureControl,
}
impl DesktopCaptureSession {
    /// Borrow actual original job completion without transferring native custody.
    pub fn job(&mut self) -> &mut OwnedJob<DesktopCaptureAttached, CaptureAttachFailure> {
        &mut self.job
    }
    /// Observe/request cleanup independently of disposable original job results.
    pub fn control(&self) -> &DesktopCaptureControl {
        &self.control
    }
}
/// Admit through the sole actual Core construction owner; fake only external effects.
pub(crate) fn reserve(
    core: &Arc<Core>,
    scope: &Scope,
    spec: JobSpec,
    input: InheritedCaptureAttachInput,
    driver: Box<dyn NativeCaptureDriver>,
    event: super::event::EventFactory,
) -> Result<DesktopCaptureSession, DesktopCaptureAdmissionRejection> {
    let run = input.run();
    let input = WorkInput { inherited: input, driver, event };
    let reject = |error, input: WorkInput| DesktopCaptureAdmissionRejection {
        error,
        input: input.inherited,
        driver: input.driver,
    };
    if spec.class != JobClass::InfrastructureLifecycle
        || spec.operation != Operation::NativeLifecycle
        || spec.replacement.is_some()
        || spec.reservation_bytes != 4_194_304
        || spec.deadline != Some(input.inherited.original_deadline())
    {
        return Err(reject(AdmissionError::InvalidCaptureAdmission, input));
    }
    core.admit_owned(
        scope,
        spec,
        RequestKind::DesktopCapture(run),
        input,
        |input, context, cancel, timing| {
            let admitted = timing.admitted;
            let work = DesktopCaptureWork::build(core, input, &context, cancel, admitted)?;
            let shared = work.shared.clone();
            let delivery = work.delivery.clone();
            let job = work.job.clone();
            let queued = QueuedJob {
                context,
                job: job.clone(),
                work: Work::Native(crate::native::NativeWork::DesktopCapture(work)),
            };
            let session = DesktopCaptureSession {
                job: OwnedJob::new(scope.id(), spec, Box::new(JobHandle(job))),
                control: DesktopCaptureControl { shared, delivery },
            };
            Ok((queued, session))
        },
    )
    .map_err(|(error, input)| reject(error, input))
}
#[cfg(windows)]
impl crate::ExecutionClient {
    /// Admit genuine inherited capture using the original counter-converted cutoff.
    /// Requires exact4MiB NativeLifecycle/InfrastructureLifecycle and the input cutoff.
    ///
    /// # Errors
    /// Returns actual admission/preallocation cause and identical complete owners.
    /// No selection, clock callback or native effect is invoked by its factory.
    pub fn reserve_desktop_capture_attachment(
        &self,
        scope: &Scope,
        spec: JobSpec,
        input: InheritedCaptureAttachInput,
        driver: Box<dyn NativeCaptureDriver>,
    ) -> Result<DesktopCaptureSession, DesktopCaptureAdmissionRejection> {
        reserve(&self.core, scope, spec, input, driver, super::event::CompletionEvent::native)
    }
}
