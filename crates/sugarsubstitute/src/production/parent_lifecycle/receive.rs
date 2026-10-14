// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain the single actual read loan across acquisition write receipts before semantic routing.
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
        if let Err(error) = self.receive(frame.as_bytes(), resources) {
            let payload_kind = received_payload_kind(frame.as_bytes());
            tracing::error!(operation="parent_received_payload_rejected",payload_kind,run=?self.run,
            native_operation=operation.get(),main_stage=?self.main.stage(),
            retained_delivery=self.output.as_ref().map(|delivery| delivery.id.get()),
            last_issued_delivery=self.next_delivery,
            native_output_rejection=?error.downcast_ref::<sugarsubstitute_execution::windows_output::WindowsOutputError>(),
            "Actual received payload rejected; original frame and error retained");
            self.rejected_frame = Some(frame);
            return Err(error);
        }
        Ok(())
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
            || self.retiring_write_pending()?)
    }
    /// Reuse the same authentic read loan after the actual progress-write receipt. This
    /// preserves its header, bytes and original cutoff; no read or request queue is added.
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
        if let Err(cause) = self.receive(frame.as_bytes(), resources) {
            tracing::error!(operation="parent_acquisition_received_payload_rejected",payload_kind=received_payload_kind(frame.as_bytes()),
                run=?self.run,native_operation=operation,main_stage=?self.main.stage(),
                native_output_rejection=?cause.downcast_ref::<sugarsubstitute_execution::windows_output::WindowsOutputError>(),
                "Original received frame and cause retained after actual progress write");
            self.rejected_frame = Some(frame);
            return Err(cause);
        }
        Ok(())
    }
    pub(super) fn receive_next(&mut self, resources: &ParentResources<'_, '_>) -> Result<()> {
        if self.read.is_some()
            || self.acquisition_receive.is_some()
            || !self.connected
            || self.closed
        {
            return Ok(());
        }
        if self.hello && (!self.welcome || self.peer.is_none()) {
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
    }
}
