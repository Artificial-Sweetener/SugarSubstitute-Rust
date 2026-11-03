// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep actual temporary outputs and the original permanent capsule on the native worker.
use super::{
    call::CallOutputs, control::DesktopCapturePhase, event::CompletionEvent,
    metadata::ChildMetadata, process_lifetime::ProcessCaptureFields, types::DesktopCaptureAttached,
    work::DesktopCaptureWork,
};
use crate::{
    admission::Core,
    capture_attach::{control::CleanupTrigger, types::CaptureAttachFailure},
    native::NativeObservationError,
    scope::lock,
    task::{self, Stop},
};
use std::sync::{Arc, Weak};
use sugarsubstitute_contracts::execution::{
    ExecutionError, JobContext, JobOutcome, Moment, PanicCause,
};

#[path = "worker/cleanup.rs"]
mod cleanup;
#[path = "worker/preparation.rs"]
mod preparation;

/// Its native worker enum keeps the SAME metadata guard outside this actual Box.
/// The real capsule is installed before any driver callback may fail or unwind.
pub(crate) struct DesktopCaptureActive {
    pub(super) work: DesktopCaptureWork,
    pub(super) context: JobContext,
    core: Weak<Core>,
    started: Moment,
    pub(super) event: Option<CompletionEvent>,
    pub(super) proof: Option<crate::capture_bundle::CaptureBundleRetirement>,
    initialized: bool,
    pub(super) input_returned: bool,
    pub(super) prepared: bool,
    pub(super) attached: bool,
    terminal: bool,
    pub(super) completed: bool,
    retirement_reported: bool,
    outcome: Option<JobOutcome<DesktopCaptureAttached, CaptureAttachFailure>>,
}
impl DesktopCaptureActive {
    /// Preclaimed metadata covers this Box before actual worker construction.
    pub(crate) fn new(
        work: DesktopCaptureWork,
        context: JobContext,
        core: &Arc<Core>,
        now: Moment,
    ) -> Box<Self> {
        lock(&work.shared.cleanup.state).active = true;
        Box::new(Self {
            work,
            context,
            core: Arc::downgrade(core),
            started: now,
            event: None,
            proof: None,
            initialized: false,
            input_returned: false,
            prepared: false,
            attached: false,
            terminal: false,
            completed: false,
            retirement_reported: false,
            outcome: None,
        })
    }
    /// Clone only the same original control claim for outside actual Box retention.
    pub(crate) fn metadata(&self) -> ChildMetadata {
        self.work.shared.metadata()
    }
    /// Actual temporary entry completion; permanent process retention is independent.
    pub(crate) fn completed(&self) -> bool {
        self.completed
    }
    /// Retain the original panic without destroying any driver/native output owner.
    pub(crate) fn fail_owner(&mut self, cause: &PanicCause) {
        self.initialized = true;
        if self.terminal {
            return;
        }
        match &mut self.outcome {
            Some(JobOutcome::Failure(CaptureAttachFailure::Native(error))) => {
                error.retain_panic(cause)
            }
            Some(JobOutcome::Failure(CaptureAttachFailure::Source(error))) => {
                error.0.retain_panic(cause)
            }
            Some(JobOutcome::Panicked(_)) => {}
            outcome => *outcome = Some(JobOutcome::Panicked(cause.clone())),
        }
    }
    /// Preserve first terminal cause and publish bounded cleanup observations separately.
    fn failed(&mut self, core: &Core, error: NativeObservationError) {
        // A stop accepted while the external stage was blocked precedes this
        // returned failure. Keep that historical outcome; the actual failure is
        // still retained in the structured mailbox and native output ledger.
        if !self.terminal && self.outcome.is_none() && self.work.shared.cleanup.stopped() {
            let reason = lock(&self.work.shared.state).stopped_outcome.unwrap_or(Stop::Cancelled);
            self.outcome = Some(task::stopped_outcome(reason));
        }
        if !self.terminal {
            match &mut self.outcome {
                Some(JobOutcome::Failure(CaptureAttachFailure::Native(primary))) => {
                    primary.retain_cleanup(error.clone())
                }
                Some(JobOutcome::Failure(CaptureAttachFailure::Source(primary))) => {
                    primary.0.retain_cleanup(error.clone())
                }
                Some(JobOutcome::Panicked(panic)) => {
                    let mut retained = error.clone();
                    retained.retain_panic(panic);
                    self.outcome =
                        Some(JobOutcome::Failure(CaptureAttachFailure::Native(retained)));
                }
                None => {
                    self.outcome =
                        Some(JobOutcome::Failure(CaptureAttachFailure::Native(error.clone())))
                }
                _ => {}
            }
        }
        let notification = error.clone();
        if let Some(capsule) = self.work.capsule.as_mut() {
            capsule.storage.failures.remember(error);
        }
        core.mailbox.record_failure(ExecutionError::new(
            self.context.spec().operation,
            Box::new(notification),
        ));
        core.mailbox.deliver_wake();
        self.work.shared.request(Stop::Cancelled, CleanupTrigger::FailedPreparation);
    }
    /// Use only current owner observations; no callback is made while locks are held.
    fn stop(&mut self, core: &Core, now: Moment) {
        let registered = self.terminal && lock(&self.work.shared.state).registered_published;
        let reason = if registered && self.work.shared.cleanup.stopped() {
            None
        } else if registered {
            match super::registered_lifetime::stop(core, &self.work, &self.context, now) {
                Ok(reason) => reason,
                Err(cause) => {
                    self.failed(core, error("capture_registered_lifetime", cause));
                    return;
                }
            }
        } else {
            task::stop_reason_at(&self.context, now).or_else(|| {
                (!self.terminal && now >= self.work.original_until).then_some(Stop::TimedOut)
            })
        };
        if let Some(reason) = reason {
            if !self.terminal && self.outcome.is_none() {
                self.outcome = Some(task::stopped_outcome(reason));
            }
            self.work.shared.request(reason, CleanupTrigger::Cancelled);
        } else if self.work.shared.cleanup.stopped() {
            let original = lock(&self.work.shared.state).stopped_outcome.unwrap_or(Stop::Cancelled);
            if !self.terminal && self.outcome.is_none() {
                self.outcome = Some(task::stopped_outcome(original));
            }
            self.work.shared.request(original, CleanupTrigger::Cancelled);
        }
        if self.outcome.is_some() && !self.terminal {
            self.work.shared.request(Stop::Cancelled, CleanupTrigger::FailedPreparation);
        }
    }
    /// Progress this exact state on the existing sole native worker.
    /// Finite provider/native work and panic resumption run outside all owner locks.
    pub(crate) fn progress(&mut self, core: &Core, now: Moment, clock_available: bool) {
        let observed = if clock_available { core.clock.now() } else { now };
        self.stop(core, observed);
        if !self.initialized && !self.work.shared.cleanup.stopped() {
            self.initialized = true;
            if let Err(cause) = self.prepare() {
                self.failed(core, cause);
            }
        }
        let observed = if clock_available { core.clock.now() } else { now };
        self.stop(core, observed);
        if self.prepared && !self.terminal && !self.work.shared.cleanup.stopped() {
            if let Err(cause) = self.advance(core) {
                self.failed(core, cause);
            }
            let observed = if clock_available { core.clock.now() } else { now };
            self.stop(core, observed);
            if !self.work.shared.cleanup.stopped() {
                self.publish_registered(core, observed);
            }
        }
        if self.work.shared.cleanup.stopped() {
            self.cleanup(core, observed);
        }
    }
    /// Borrow only the real preallocated occupied capsule on this worker.
    fn fields(&self) -> Option<&ProcessCaptureFields> {
        self.work.capsule.as_ref()?.storage.fields.as_deref()
    }
    /// Native partial outputs remain in this actual Box even if callback unwinds.
    fn fields_mut(&mut self) -> Option<&mut ProcessCaptureFields> {
        self.work.capsule.as_mut()?.storage.fields.as_deref_mut()
    }
    /// Commit registration only through the same Core terminal publication fence.
    fn publish_registered(&mut self, core: &Core, now: Moment) {
        let Some(fields) = self.fields() else {
            return;
        };
        let facts = match super::native_reply::decode(&fields.outputs.outer) {
            Ok(facts) => facts,
            Err(cause) => {
                self.failed(core, error("capture_registered_reply", cause));
                return;
            }
        };
        if !facts.registered {
            return;
        }
        match super::publication::registered(
            core,
            &self.work.job,
            &self.work.shared,
            &self.context,
            self.started,
            now,
            &fields.outputs,
        ) {
            Ok(true) => self.terminal = true,
            Ok(false) => self.failed(core, invariant("capture_registered_delivery")),
            Err(cause) => self.failed(core, cause),
        }
    }
    /// Publish one original nonsuccess outcome without rewriting historical registration.
    fn publish_failure(&mut self, now: Moment) {
        if self.terminal {
            return;
        }
        if let Some(outcome) = self.outcome.take() {
            self.terminal = true;
            self.work.job.finish_in_custody_at(&self.context, Some(self.started), now, outcome);
        }
    }
}
/// Preserve the actual typed producer cause without a synthetic native error code.
fn error(
    stage: &'static str,
    cause: impl std::error::Error + Send + Sync + 'static,
) -> NativeObservationError {
    NativeObservationError::new(stage, cause)
}
/// Missing owner fields never manufacture checked retirement or a replacement lease.
fn invariant(stage: &'static str) -> NativeObservationError {
    error(stage, std::io::Error::other("Actual child capture owner unavailable"))
}
