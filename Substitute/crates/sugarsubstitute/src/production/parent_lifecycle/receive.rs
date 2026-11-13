// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain one actual read loan across acquisition receipts or shared output loan contention.
use super::{ParentError, ParentLifecycle, ParentResources, Result, types::WritePurpose};
use sugarsubstitute_execution::windows_lifecycle_pipe::{
    LifecyclePipeError, LifecyclePipeOperationId, ReceivedLifecycleFrame,
};
use sugarsubstitute_supervision::lifecycle_protocol::FrameCodec;
impl ParentLifecycle {
    /// Validate native operation correlation before retaining or routing its authentic frame.
    pub(super) fn observe_received(
        &mut self,
        operation: LifecyclePipeOperationId,
        frame: ReceivedLifecycleFrame,
        resources: &mut ParentResources<'_, '_>,
    ) -> Result<()> {
        if self.read != Some(operation.get()) || self.rejected_frame.is_some() {
            tracing::error!(operation="parent_received_native_operation_rejected",
            payload_kind=received_payload_kind(frame.as_bytes()),run=?self.run,
            native_operation=operation.get(),expected_operation=self.read,
            retained_frame=self.rejected_frame.is_some(),
            "Actual native receive correlation rejected; original frame retained");
            self.rejected_frame = Some(frame);
            return Err(ParentError::WrongPhase.into());
        }
        self.read = None;
        if self.hold_acquisition_receive()? {
            if self.acquisition_receive.is_some() {
                self.rejected_frame = Some(frame);
                return Err(ParentError::WrongPhase.into());
            }
            tracing::debug!(operation="parent_receive_waits_acquisition_receipt",payload_kind=received_payload_kind(frame.as_bytes()),
            run=?self.run,native_operation=operation.get(),
            "Original read loan retained until actual acquisition progress write receipt");
            self.acquisition_receive = Some((operation.get(), frame));
            return Ok(());
        }
        match self.receive(frame.as_bytes(), resources) {
            Ok(true) => Ok(()),
            Ok(false) => {
                self.acquisition_receive = Some((operation.get(), frame));
                Ok(())
            }
            Err(error) => {
                let payload_kind = received_payload_kind(frame.as_bytes());
                tracing::error!(operation="parent_received_payload_rejected",payload_kind,run=?self.run,
            native_operation=operation.get(),main_stage=?self.main.stage(),
            retained_delivery=self.output.as_ref().map(|delivery| delivery.id.get()),
            last_issued_delivery=self.next_delivery,
            native_output_rejection=?error.downcast_ref::<sugarsubstitute_execution::windows_output::WindowsOutputError>(),
            "Actual received payload rejected; original frame and error retained");
                self.rejected_frame = Some(frame);
                Err(error)
            }
        }
    }
    /// Retain the existing read loan while progress is writing, or while an original
    /// Grant must finish before cancellation can close its ready resource. No capability
    /// is admitted by this receipt-state check; full admission runs after the checked receipt.
    pub(super) fn hold_acquisition_receive(&self) -> Result<bool> {
        Ok(self.write.as_ref().is_some_and(|write| progress_write(&write.purpose))
            || self
                .acquisition
                .as_ref()
                .is_some_and(super::acquisition::Acquisition::awaiting_grant_write)
            || self
                .recovery
                .handoff
                .is_some_and(|handoff| handoff.header.is_some() && !handoff.written)
            || self.retiring_write_pending()?)
    }
    /// Reconsider the same authentic frame after receipt or output-capacity activity.
    /// Normal admission still validates its original header, bytes, authority and cutoff.
    /// A contended output pull retains the frame and prevents another native read.
    pub(super) fn resume_acquisition_receive(
        &mut self,
        resources: &mut ParentResources<'_, '_>,
    ) -> Result<()> {
        if self.closed || (self.acquisition_receive.is_some() && self.hold_acquisition_receive()?) {
            return Ok(());
        }
        let Some((operation, frame)) = self.acquisition_receive.take() else {
            return Ok(());
        };
        match self.receive(frame.as_bytes(), resources) {
            Ok(true) => Ok(()),
            Ok(false) => {
                self.acquisition_receive = Some((operation, frame));
                Ok(())
            }
            Err(cause) => {
                tracing::error!(operation="parent_acquisition_received_payload_rejected",payload_kind=received_payload_kind(frame.as_bytes()),
                run=?self.run,native_operation=operation,main_stage=?self.main.stage(),
                native_output_rejection=?cause.downcast_ref::<sugarsubstitute_execution::windows_output::WindowsOutputError>(),
                "Original deferred frame and cause retained after admission reconsideration");
                self.rejected_frame = Some(frame);
                Err(cause)
            }
        }
    }
    /// Borrow this actor's receive-admission decision without native effects or new timing.
    /// A validated final ExitCompleted request latches its reply index before ACK issue.
    /// No further request is expected while that exact ACK finishes; explicit pipe
    /// closure still waits for its original full Written receipt.
    pub(super) fn receive_admission_open(&self) -> bool {
        self.read.is_none()
            && self.acquisition_receive.is_none()
            && self.output_receive_until.is_none()
            && self.connected
            && !self.closed
            && self.close_after_reply.is_none()
            && (!self.hello || (self.welcome && self.peer.is_some()))
    }
    pub(super) fn receive_next(&mut self, resources: &ParentResources<'_, '_>) -> Result<()> {
        if !self.receive_admission_open() {
            return Ok(());
        }
        let next = self.next_operation()?;
        let control = &resources.session.control;
        let admitted = if self.registered {
            control.read_passive(LifecyclePipeOperationId::new(next)?)
        } else {
            control.read(LifecyclePipeOperationId::new(next)?, control.original_deadline())
        };
        match admitted {
            Ok(()) => {
                self.latest_operation = next;
                self.read = Some(next);
            }
            Err(LifecyclePipeError::Busy) => {}
            Err(error) => return Err(error.into()),
        }
        Ok(())
    }
}

/// Only the original acquisition progress writer fences receive routing; queued or
/// issued metadata still grants no output permission before checked completion.
fn progress_write(purpose: &WritePurpose) -> bool {
    matches!(purpose, WritePurpose::AcquisitionProgress(_))
}

/// Classify borrowed frame shape only at a retained failure boundary. No payload,
/// credential, path or capability bytes enter diagnostics; this grants no authority.
fn received_payload_kind(bytes: &[u8]) -> &'static str {
    use sugarsubstitute_supervision::lifecycle_protocol::DecodedPayload as Payload;
    let Ok(message) = FrameCodec::decode(bytes) else {
        return "Malformed";
    };
    match message.payload {
        Payload::Hello(_) => "Hello",
        Payload::Welcome { .. } => "Welcome",
        Payload::Register => "Register",
        Payload::Acquire { .. } => "Acquire",
        Payload::Release(_) => "Release",
        Payload::Reload { .. } => "Reload",
        Payload::ExitIntent { .. } => "ExitIntent",
        Payload::ExitCompleted(_) => "ExitCompleted",
        Payload::UiChallenge(_) => "UiChallenge",
        Payload::UiResponse(_) => "UiResponse",
        Payload::Reply(_) => "Reply",
        Payload::RequestedProfileFolder(_) => "RequestedProfileFolder",
        Payload::EnvironmentPrepared(_) => "EnvironmentPrepared",
        Payload::ControllerPrepared { .. } => "ControllerPrepared",
        Payload::MainRetired(_) => "MainRetired",
        Payload::NativeActivity { .. } => "NativeActivity",
        Payload::OutputPull { .. } => "OutputPull",
        Payload::MainFailed(_) => "MainFailed",
        Payload::SettleAttempt { .. } => "SettleAttempt",
        Payload::RetryBackend { .. } => "RetryBackend",
        Payload::MainRevealed(_) => "MainRevealed",
        Payload::UpdateStatus { .. } => "UpdateStatus",
        Payload::RestartToApply => "RestartToApply",
    }
}
