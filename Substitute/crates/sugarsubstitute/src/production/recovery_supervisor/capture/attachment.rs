// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit and observe one attachment pin for the exact Root-delegated successor Desktop.
use super::*;
#[derive(Clone, Debug)]
pub(super) struct AttachmentCause(Arc<CaptureAttachFailure>);
impl std::fmt::Display for AttachmentCause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self.0.as_ref(), f)
    }
}
impl Error for AttachmentCause {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.0.as_ref())
    }
}
impl Capture {
    /// The source, current actual registration and exact target must all precede native attachment.
    pub(in crate::production::recovery_supervisor) fn attach(
        &mut self,
        client: &ExecutionClient,
        native: &Scope,
        jobs: &JobSequence,
    ) -> Result<()> {
        if self.attachment_original.is_some() || !self.ready() {
            return Err("Fresh Desktop attachment repeated or unavailable".into());
        }
        self.origin.check()?;
        let build =
            self.registration.registration_build().ok_or("Fresh selected capture build missing")?;
        let build = BuildIdentity::new(build.as_str()?)?;
        let creator =
            self.registration.creator_process().ok_or("Fresh actual capture creator missing")?;
        let handler =
            self.registration.snapshot().handler.ok_or("Fresh actual capture handler missing")?;
        let handler = CaptureProcessFacts::new(handler.process_id, handler.creation_time_100ns)?;
        let budget = CaptureBudget::registration(client.now()?, self.origin.original_until())?;
        let expected = CaptureAttachExpectation::new(
            self.origin.key().run().owner,
            CaptureTarget::Desktop(self.origin.key().run()),
            &build,
            creator,
            handler,
            budget,
        )?;
        let spec = JobSpec {
            id: jobs.allocate()?,
            operation: Operation::NativeLifecycle,
            class: JobClass::NativeObservation,
            deadline: None,
            reservation_bytes: 16384,
            replacement: None,
        };
        self.attachment =
            Some(self.registration.request_attach_bootstrap(client, native, spec, expected)?);
        self.attachment_original = Some((native.id(), spec));
        Ok(())
    }
    /// Keep original terminal metadata and full failure causes after moving the successful pin.
    pub(super) fn observe_attachment(&mut self) -> Result<()> {
        if self.attachment_terminal.is_some() {
            return Ok(());
        }
        let Some(done) = self.attachment.as_mut().and_then(OwnedJob::try_take_completion) else {
            return Ok(());
        };
        let valid = self.attachment_original == Some((done.scope_id, done.spec))
            && done.memory.bytes() == 16384
            && done.native_handles.is_none();
        let JobCompletion {
            scope_id,
            spec,
            queue_time,
            execution_time,
            outcome,
            memory,
            native_handles,
        } = done;
        let outcome = match outcome {
            JobOutcome::Success(pin) => {
                self.pin = Some(pin);
                JobOutcome::Success(())
            }
            JobOutcome::Failure(cause) => JobOutcome::Failure(AttachmentCause(Arc::new(cause))),
            JobOutcome::Panicked(cause) => JobOutcome::Panicked(cause),
            JobOutcome::Cancelled => JobOutcome::Cancelled,
            JobOutcome::TimedOut => JobOutcome::TimedOut,
            JobOutcome::Coalesced { replacement } => JobOutcome::Coalesced { replacement },
        };
        self.attachment_terminal = Some(JobCompletion {
            scope_id,
            spec,
            queue_time,
            execution_time,
            outcome,
            memory,
            native_handles,
        });
        self.attachment = None;
        if !valid {
            return Err("Fresh attachment original completion rejected".into());
        }
        super::super::channel::terminal::classify(
            &self.attachment_terminal.as_ref().ok_or("Fresh attachment terminal absent")?.outcome,
            self.closing,
        )
    }
}
