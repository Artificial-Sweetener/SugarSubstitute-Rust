// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain the fresh process registration and the one exact Desktop attachment admission.
#[path = "capture/attachment.rs"]
mod attachment;
#[path = "capture/retirement.rs"]
mod retirement;
use crate::job_sequence::JobSequence;
use std::{error::Error, sync::Arc};
use sugarsubstitute_contracts::{
    crash::{CaptureBudget, CaptureState, CaptureTarget},
    execution::*,
    incident::BuildIdentity,
    lifecycle::ShutdownBudget,
    recovery_supervisor::RecoveryProcessIdentity,
};
use sugarsubstitute_crash::{CaptureStartInput, WindowsCrashCapture};
use sugarsubstitute_execution::{
    ExecutionClient,
    recovery_supervisor::AuthenticatedRecoveryOrigin,
    windows::{NativeCorrelation, NativeObservationError, ProcessExit},
    windows_capture_attach::{
        CaptureAttachExpectation, CaptureAttachFailure, CaptureAttachLaunchPin, CaptureProcessFacts,
    },
};
use sugarsubstitute_supervision::InstalledReleaseOwner;
type Result<T> = std::result::Result<T, Box<dyn Error>>;
type Attachment = JobCompletion<(), attachment::AttachmentCause>;
/// Registration remains subordinate to the original authenticated Root run.
/// This owner never creates coordinator, backend, profile or diagnostic-writer authority.
pub(super) struct Capture {
    registration: WindowsCrashCapture,
    origin: AuthenticatedRecoveryOrigin,
    rejected_input: Option<CaptureStartInput>,
    unresolved_rejection: bool,
    registration_admitted: bool,
    unbound_watch: Option<OwnedJob<ProcessExit, NativeObservationError>>,
    unbound_terminal: Option<JobCompletion<ProcessExit, NativeObservationError>>,
    attachment: Option<OwnedJob<CaptureAttachLaunchPin, CaptureAttachFailure>>,
    attachment_original: Option<(ScopeId, JobSpec)>,
    attachment_terminal: Option<Attachment>,
    pin: Option<CaptureAttachLaunchPin>,
    start_attempted: bool,
    reported_failure: bool,
    closing: bool,
}
impl Capture {
    /// Bind a new process-local registration to the sealed original Root source.
    pub(super) fn new(origin: AuthenticatedRecoveryOrigin) -> Self {
        Self {
            registration: WindowsCrashCapture::new(origin.key().run().owner),
            origin,
            rejected_input: None,
            unresolved_rejection: false,
            registration_admitted: false,
            unbound_watch: None,
            unbound_terminal: None,
            attachment: None,
            attachment_original: None,
            attachment_terminal: None,
            pin: None,
            start_attempted: false,
            reported_failure: false,
            closing: false,
        }
    }
    /// Select the actual current installed release and protected derived generation once.
    /// The exact original30-second grant bounds registration even though native capture has60s.
    pub(super) fn start(
        &mut self,
        client: &ExecutionClient,
        native: &Scope,
        finite: &Scope,
        jobs: &JobSequence,
    ) -> Result<()> {
        if self.start_attempted || self.closing {
            return Err("Fresh capture start repeated or closed".into());
        }
        self.origin.check()?;
        let input = CaptureStartInput::recovery(
            self.origin.clone(),
            InstalledReleaseOwner::new().capture_selection(),
        )?;
        let budget = CaptureBudget::new(client.now()?)?;
        let watch_id = jobs.allocate()?;
        let watch = client.reserve_process_watch(
            native,
            JobSpec {
                id: watch_id,
                operation: Operation::NativeLifecycle,
                class: JobClass::NativeObservation,
                deadline: None,
                reservation_bytes: 4096,
                replacement: None,
            },
            NativeCorrelation::new(watch_id.get())?,
        )?;
        let spec = JobSpec {
            id: jobs.allocate()?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(self.origin.original_until().min(budget.until())),
            reservation_bytes: input.required_reservation_bytes() as u64,
            replacement: None,
        };
        self.start_attempted = true;
        if let Err(rejected) =
            self.registration.submit_start(client, finite, spec, budget, watch, input)
        {
            match rejected.into_parts() {
                Ok((cause, watch, input)) => {
                    self.unbound_watch = Some(watch.cancel_unbound());
                    self.rejected_input = Some(input);
                    return Err(Box::new(cause));
                }
                Err(cause) => {
                    self.unresolved_rejection = true;
                    return Err(Box::new(cause));
                }
            }
        }
        self.registration_admitted = true;
        Ok(())
    }
    /// Observe every actual owner before delivering the first retained native/provider failure.
    pub(super) fn observe(&mut self, batch: &CompletionBatch) -> Result<()> {
        self.registration.observe(batch);
        let attachment = self.observe_attachment();
        if !self.reported_failure
            && let Some(cause) = self.registration.errors().next()
        {
            self.reported_failure = true;
            if let Err(secondary) = attachment {
                tracing::error!(operation="recovery_capture_attachment",cause=%secondary,"Additional actual attachment failure retained");
            }
            return Err(Box::new(cause.clone()));
        }
        attachment
    }
    /// Preserve conservative capture time after any actual start admission, including partial outputs.
    pub(super) const fn attempted(&self) -> bool {
        self.registration_admitted
    }
    /// Actual registered and still-watched handler readiness, separate from Desktop attachment.
    pub(super) fn ready(&self) -> bool {
        !self.closing
            && self.registration.snapshot().state == CaptureState::Registered
            && self.registration.snapshot().handler_exit.is_none()
    }
    /// Return exact kernel identities read from this actual registration's retained processes.
    pub(super) fn identities(&self) -> Result<(RecoveryProcessIdentity, RecoveryProcessIdentity)> {
        if !self.ready() {
            return Err("Fresh capture registration unavailable".into());
        }
        let creator = self.registration.creator_process().ok_or("Fresh capture creator missing")?;
        let handler =
            self.registration.snapshot().handler.ok_or("Fresh capture handler missing")?;
        Ok((
            RecoveryProcessIdentity::new(creator.process_id(), creator.creation_time_100ns())?,
            RecoveryProcessIdentity::new(handler.process_id, handler.creation_time_100ns)?,
        ))
    }
    /// Move the genuine same-registration launch pin once; no process facts can construct it.
    pub(super) fn take_pin(&mut self) -> Option<CaptureAttachLaunchPin> {
        self.pin.take()
    }
}
