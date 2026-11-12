// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Connect fixed request correlation to the same admitted execution and transport.
use super::*;

impl ChildChannel {
    /// Inspect routing identity while retaining the original received frame and charge.
    /// Each semantic consumer must still validate its own payload and request association.
    /// # Errors
    /// Preserves malformed frame decoding causes without transferring received custody.
    pub(in super::super) fn received_header(&self) -> Result<Option<WireHeader>> {
        let Some(frame) = &self.received else {
            return Ok(None);
        };
        Ok(Some(FrameCodec::decode(frame.as_bytes())?.header))
    }

    /// Encode and retain one original cutoff through this channel's actual clock owner.
    /// # Errors
    /// Rejects closed/unregistered channels, clock failure, expiry or fixed-slot exhaustion.
    pub(in super::super) fn prepare_request(&mut self, original: Moment) -> Result<WireHeader> {
        if !self.admission.open() {
            return Err(Box::new(LifecyclePipeError::Closed));
        }
        if self.requests.is_none() {
            return Err("Lifecycle request preparation precedes registration".into());
        }
        let encoded = self
            .client
            .as_ref()
            .ok_or("Original lifecycle execution client missing")?
            .encode_counter_deadline(original)?;
        Ok(self
            .requests
            .as_mut()
            .ok_or("Original request ledger missing")?
            .prepare(original, encoded)?)
    }

    /// Admit a prepared application request using its unchanged local/counter pair.
    /// False retains the original request through bounded native transport contention.
    /// Semantic owners validate payload identities and native privileges separately.
    /// # Errors
    /// Preserves clock, correlation, encoding and actual native admission causes.
    pub(in super::super) fn try_send(
        &mut self,
        message: &WireMessage<'_>,
        original: Moment,
    ) -> Result<bool> {
        if !self.admission.open() {
            return Err(Box::new(LifecyclePipeError::Closed));
        }
        let binding =
            self.binding().ok_or("Lifecycle application request precedes registration")?;
        wire::outbound(binding, message)?;
        if let WirePayload::UiResponse(id) = message.payload {
            self.ui.as_ref().ok_or("Original UI exchange missing")?.validate_response(
                message.header,
                original,
                id,
            )?;
        }
        let now =
            self.client.as_ref().ok_or("Original lifecycle execution client missing")?.now()?;
        let ledger = self.requests.as_ref().ok_or("Original request ledger missing")?;
        match ledger.outbound(message.header, original, now) {
            Ok(true) => {}
            Ok(false) => return Ok(false),
            Err(cause) => {
                return Err(match ledger.expiry(message.header, cause) {
                    Ok(expiry) => Box::new(expiry),
                    Err(cause) => Box::new(cause),
                });
            }
        }
        let accepted = Self::admit_write(
            self.session.as_ref().ok_or("Lifecycle application session missing")?,
            &mut self.latest,
            &mut self.write,
            message,
            original,
        )?;
        if accepted {
            self.requests
                .as_mut()
                .ok_or("Original request ledger missing")?
                .admitted(message.header)?;
        }
        Ok(accepted)
    }

    /// Mark the exact prepared Acquire before native admission; no payload authority is granted.
    pub(in super::super) fn acquisition(&mut self, header: WireHeader) -> Result<()> {
        self.requests.as_mut().ok_or("Original request ledger missing")?.acquisition(header)?;
        Ok(())
    }
    /// Consume only validated nonfinal Acquire progress while preserving its original slot.
    pub(in super::super) fn consume_progress(&mut self, header: WireHeader) -> Result<()> {
        let now = self.client.as_ref().ok_or("Original lifecycle client missing")?.now()?;
        self.requests.as_mut().ok_or("Original request ledger missing")?.progress(header, now)?;
        Ok(())
    }
    /// Release only matching metadata after the semantic owner handles the original reply.
    /// Frame/native custody and positive application readiness remain separate.
    /// # Errors
    /// Rejects unsolicited or unreceived replies without erasing original evidence.
    pub(in super::super) fn consume_reply(&mut self, header: WireHeader) -> Result<()> {
        self.requests.as_mut().ok_or("Original request ledger missing")?.consumed(header)?;
        Ok(())
    }

    /// Withdraw only this ledger's never-admitted request; no write or native loan is retired.
    pub(in super::super) fn withdraw_prepared(&mut self, header: WireHeader) -> Result<()> {
        self.requests
            .as_mut()
            .ok_or("Original request ledger missing")?
            .withdraw_prepared(header)?;
        Ok(())
    }
    /// Handle one actual server frame without copying its charged payload.
    /// True transfers it to the application; false completes inline watchdog handling.
    /// The caller retains the original read guard on every semantic failure.
    pub(super) fn handle_application(&mut self, frame: &ReceivedLifecycleFrame) -> Result<bool> {
        let frame = FrameCodec::decode(frame.as_bytes())?;
        let binding = self.binding().ok_or("Lifecycle application frame precedes registration")?;
        match wire::incoming(binding, &frame)? {
            wire::IncomingKind::Reply => {
                let now = self
                    .client
                    .as_ref()
                    .ok_or("Original lifecycle execution client missing")?
                    .now()?;
                let ledger = self.requests.as_mut().ok_or("Original request ledger missing")?;
                let diagnostic = self.ui.as_ref().is_some_and(|ui| ui.matches_reply(frame.header));
                let received = if diagnostic {
                    ledger.received_diagnostic(frame.header)
                } else {
                    ledger.received_or_retiring(frame.header, now)
                };
                if let Err(cause) = received {
                    return Err(match ledger.expiry(frame.header, cause) {
                        Ok(expiry) => Box::new(expiry),
                        Err(cause) => Box::new(cause),
                    });
                }
                if diagnostic {
                    let sugarsubstitute_supervision::lifecycle_protocol::DecodedPayload::Reply(
                        outcome,
                    ) = frame.payload
                    else {
                        return Err("Original UI response reply payload changed".into());
                    };
                    self.ui.as_mut().ok_or("Original UI exchange missing")?.acknowledged(
                        self.requests.as_mut().ok_or("Original request ledger missing")?,
                        frame.header,
                        outcome,
                    )?;
                    return Ok(false);
                }
            }
            wire::IncomingKind::Challenge => {
                self.receive_ui_challenge(&frame)?;
                return Ok(false);
            }
            wire::IncomingKind::UpdateStatus => {
                self.updates.receive(&frame.payload)?;
                return Ok(false);
            }
        }
        Ok(true)
    }
}
