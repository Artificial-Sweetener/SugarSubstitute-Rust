// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain the one genuine registered-child development fault job and original terminal custody.
use super::super::qualification::QualificationIntent;
#[cfg(feature = "k1-native-qualification")]
#[path = "preview_hooks.rs"]
mod preview_hooks;

/// Inert spelling cannot arm capture; the existing central owner authenticates the real session.
pub(super) struct ChildQualification {
    intent: QualificationIntent,
    #[cfg(feature = "k1-native-qualification")]
    fault: Fault,
    #[cfg(feature = "k1-native-qualification")]
    preview: preview_hooks::PreviewHooks,
}
impl ChildQualification {
    /// Construct before authentication without admitting or starting native work.
    pub(super) fn new(intent: QualificationIntent) -> Self {
        Self {
            intent,
            #[cfg(feature = "k1-native-qualification")]
            fault: Fault::default(),
            #[cfg(feature = "k1-native-qualification")]
            preview: preview_hooks::PreviewHooks::default(),
        }
    }
}
#[cfg(feature = "k1-native-qualification")]
use sugarsubstitute_contracts::{execution::*, lifecycle::RunKey};
#[cfg(feature = "k1-native-qualification")]
use sugarsubstitute_execution::{
    DesktopCaptureSession, ExecutionClient, RegisteredDesktopFaultError,
};
#[cfg(feature = "k1-native-qualification")]
type FaultCompletion = JobCompletion<(), RegisteredDesktopFaultError>;
#[cfg(feature = "k1-native-qualification")]
#[derive(Default)]
struct Fault {
    spec: Option<JobSpec>,
    original: Option<(ExecutionId, ScopeId, RunKey, Moment)>,
    job: Option<OwnedJob<(), RegisteredDesktopFaultError>>,
    completion: Option<FaultCompletion>,
    admission: Option<AdmissionError>,
    issued: bool,
    invalid: bool,
}
#[cfg(feature = "k1-native-qualification")]
impl ChildQualification {
    /// Submit once only after genuine capture readiness and the original parent registration.
    /// Busy preserves the same request and original cutoff; successful admission consumes it.
    pub(super) fn advance_fault(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &crate::job_sequence::JobSequence,
        session: &DesktopCaptureSession,
        run: RunKey,
        until: Moment,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if !self.intent.fault() || self.fault.issued {
            return Ok(());
        }
        if self.fault.invalid || self.fault.admission.as_ref().is_some_and(|cause| !overload(cause))
        {
            return Err("Original registered fault failure remains retained".into());
        }
        let original = (scope.owner_id(), scope.id(), run, until);
        if self.fault.original.is_some_and(|old| old != original) {
            self.fault.invalid = true;
            return Err("Original registered fault source changed".into());
        }
        self.fault.original = Some(original);
        if self.fault.spec.is_none() {
            self.fault.spec = Some(JobSpec {
                id: jobs.allocate()?,
                operation: Operation::NativeLifecycle,
                class: JobClass::InfrastructureLifecycle,
                deadline: Some(until),
                reservation_bytes: 4096,
                replacement: None,
            });
        }
        let spec = self.fault.spec.ok_or("Original registered fault specification missing")?;
        match client.submit_registered_desktop_fault(scope, spec, session, run) {
            Ok(job) => {
                self.fault.job = Some(job);
                self.fault.issued = true;
                if self.fault.admission.as_ref().is_some_and(overload) {
                    self.fault.admission = None;
                }
                Ok(())
            }
            Err(cause) => {
                let busy = overload(&cause);
                if busy {
                    self.fault.admission.get_or_insert(cause);
                    Ok(())
                } else {
                    self.fault.admission = Some(cause);
                    Err("Original registered fault admission rejected".into())
                }
            }
        }
    }
    /// Retain only genuine matching central delivery, including all original completion guards.
    /// Any returned terminal means the deliberate fault failed to retire this actual process.
    pub(super) fn capture(
        &mut self,
        batch: &CompletionBatch,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.capture_preview(batch);
        let Some(job) = &mut self.fault.job else {
            return Ok(());
        };
        let Some(notice) = batch
            .notifications
            .iter()
            .find(|notice| notice.scope_id == job.scope_id() && notice.job_id == job.spec().id)
        else {
            return Ok(());
        };
        let Some(done) = job.try_take_completion() else {
            return Ok(());
        };
        self.fault.invalid |= done.scope_id != job.scope_id()
            || done.spec != job.spec()
            || done.outcome.kind() != notice.kind
            || notice.replacement.is_some()
            || done.memory.bytes() != 4096
            || done.native_handles.is_some();
        self.fault.completion = Some(done);
        self.fault.job = None;
        if self.fault.invalid {
            Err("Original registered fault terminal identity rejected".into())
        } else {
            Err("Original registered fault returned with retained actual terminal".into())
        }
    }
    /// Observe the real owned job; cancellation is not its terminal completion.
    pub(super) fn pending(&self) -> bool {
        self.fault.job.is_some() || self.preview_pending()
    }
    /// Cancel the actual queued/active job while keeping its real terminal observable.
    pub(super) fn close(&mut self) {
        self.close_preview();
        if let Some(job) = &self.fault.job {
            job.cancel();
        }
    }
    /// Borrow the exact first rejection or real fault outcome; no external formatter is invoked.
    pub(super) fn failure(&self) -> Option<&(dyn std::error::Error + 'static)> {
        if let Some(done) = &self.fault.completion
            && let JobOutcome::Failure(cause) = &done.outcome
        {
            return Some(cause);
        }
        self.fault.admission.as_ref().map(|cause| cause as &(dyn std::error::Error + 'static))
    }
    /// A genuine terminal retains its original result; no capture or success is synthesized.
    pub(super) fn fault_requested(&self) -> bool {
        self.intent.fault()
    }
}

#[cfg(feature = "k1-native-qualification")]
fn overload(cause: &AdmissionError) -> bool {
    matches!(
        cause,
        AdmissionError::QueueSaturated { .. }
            | AdmissionError::MemorySaturated { .. }
            | AdmissionError::CompletionSaturated
    )
}
