// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain one immutable authenticated request through native write contention and exact reply consumption.
use super::*;
impl ProductionChild {
    pub(in super::super) fn advance_control(&mut self) -> Result<()> {
        if !self.bootstrap.channel_mut().admitted() {
            return Ok(());
        }
        if self.shutdown.closing
            && let Some(header) = self.bootstrap.channel_mut().received_header()?
            && self.shutdown.pending.is_none_or(|pending| pending.header != header)
            && !self.output.owns_reply(header)
            && self.acquire.is_none_or(|acquire| acquire.header != header)
        {
            let frame = self
                .bootstrap
                .channel_mut()
                .take_frame()
                .ok_or("Actual abandoned reply disappeared")?;
            self.failed_frame = Some(frame);
            let decoded = FrameCodec::decode(
                self.failed_frame.as_ref().ok_or("Retained abandoned reply absent")?.as_bytes(),
            )?;
            if !matches!(decoded.payload, DecodedPayload::Reply(_)) {
                return Err("Abandoned frame is not an authenticated original reply".into());
            }
            self.bootstrap.channel_mut().consume_reply(decoded.header)?;
            self.failed_frame = None;
        }
        if let Some(pending) = self.shutdown.pending
            && pending.admitted
            && self.bootstrap.channel_mut().received_header()? == Some(pending.header)
        {
            let frame = self
                .bootstrap
                .channel_mut()
                .take_frame()
                .ok_or("Actual lifecycle reply missing")?;
            self.failed_frame = Some(frame);
            let decoded = FrameCodec::decode(
                self.failed_frame.as_ref().ok_or("Retained lifecycle reply absent")?.as_bytes(),
            )?;
            let acknowledged = match pending.control {
                Control::Release(_) => {
                    matches!(decoded.payload, DecodedPayload::Reply(ReplyOutcome::Released))
                }
                _ => matches!(decoded.payload, DecodedPayload::Reply(ReplyOutcome::Acknowledged)),
            };
            if decoded.header != pending.header || !acknowledged {
                return Err("Actual lifecycle disposition was not acknowledged".into());
            }
            self.bootstrap.channel_mut().consume_reply(decoded.header)?;
            match pending.control {
                Control::Release(_) => self.shutdown.released = true,
                Control::MainRetired(_) => self.shutdown.main_retirement_ack = true,
                Control::ExitIntent => self.shutdown.intent_done = true,
                Control::ExitCompleted => self.shutdown.completed_done = true,
                _ => {}
            }
            self.shutdown.pending = None;
            self.failed_frame = None;
        }
        if self.shutdown.pending.is_none()
            && let Some(control) = take_first(&mut self.shutdown.queued)
        {
            let original = if let Some(budget) = self.shutdown.budget {
                if matches!(control, Control::ExitIntent) {
                    budget.graceful_until
                } else {
                    budget.settle_until
                }
            } else if matches!(control, Control::MainRetired(_)) && self.attempt.is_some() {
                self.attempt.as_ref().ok_or("Original attempt absent")?.original_deadline()
            } else {
                self.client()?.now()?.checked_add(Duration::from_secs(30))?
            };
            let header = self.bootstrap.channel_mut().prepare_request(original)?;
            self.shutdown.pending = Some(Pending { header, original, control, admitted: false });
        }
        if let Some(pending) = self.shutdown.pending
            && !pending.admitted
        {
            let payload = match pending.control {
                Control::Activity { revision, activity } => {
                    WirePayload::NativeActivity { revision, activity }
                }
                Control::MainRetired(identity) => WirePayload::MainRetired(identity),
                Control::Release(lease) => WirePayload::Release(lease),
                Control::ExitIntent => WirePayload::ExitIntent {
                    shutdown: self.shutdown.shutdown.ok_or("Actual shutdown identity absent")?,
                    intent: self.completion_intent(),
                    expected_code: 0,
                },
                Control::ExitCompleted => WirePayload::ExitCompleted(
                    self.shutdown.shutdown.ok_or("Actual shutdown identity absent")?,
                ),
            };
            if self
                .bootstrap
                .channel_mut()
                .try_send(&WireMessage { header: pending.header, payload }, pending.original)?
            {
                self.shutdown
                    .pending
                    .as_mut()
                    .ok_or("Original control request disappeared")?
                    .admitted = true;
            }
        }
        Ok(())
    }
    pub(in super::super) fn completion_intent(&self) -> CompletionIntent {
        if self.failed || self.failure.is_some() {
            CompletionIntent::StartupFailed
        } else if self.phase == Some(sugarsubstitute_application::startup::StartupPhase::Running) {
            CompletionIntent::Normal
        } else {
            CompletionIntent::StartupCancelled
        }
    }
}
