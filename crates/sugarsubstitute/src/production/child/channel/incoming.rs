// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drain real observations while preserving original transport/frame custody.
use super::*;

impl ChildChannel {
    pub(super) fn observe(&mut self, observation: LifecyclePipeObservation) -> Result<()> {
        let control = &self.session.as_ref().ok_or("Production lifecycle session missing")?.control;
        if !self.admission.open()
            && matches!(
                observation,
                LifecyclePipeObservation::Prepared
                    | LifecyclePipeObservation::Connected
                    | LifecyclePipeObservation::Authenticated(_)
            )
        {
            return Ok(());
        }
        match observation {
            LifecyclePipeObservation::Prepared => {
                if self.handshake.is_some() {
                    return Err("Native lifecycle preparation repeated".into());
                }
                let run = control
                    .prepared_run()
                    .ok_or("Native lifecycle bootstrap lacks prepared run")?;
                if run != self.expected_run {
                    return Err(
                        "Actual lifecycle bootstrap differs from original desktop run".into()
                    );
                }
                self.capability = control.take_client_capability()?;
                if self.capability.is_none() {
                    return Err("Native lifecycle bootstrap lacks capability custody".into());
                }
                self.handshake = Some(WireHandshake::new(
                    run,
                    CounterDeadline::new(self.original.ticks(), self.original.frequency())?,
                ));
            }
            LifecyclePipeObservation::Connected => {
                if self.connected {
                    return Err("Native lifecycle connection repeated".into());
                }
                self.connected = true;
                if self.admission.open() {
                    let capability =
                        self.capability.as_ref().ok_or("Native lifecycle capability missing")?;
                    control.verify_capability(*capability.bytes(), control.original_deadline())?;
                }
            }
            LifecyclePipeObservation::Authenticated(peer) => {
                let binding =
                    self.handshake.as_ref().ok_or("Original prepared lifecycle run missing")?;
                if self.peer.is_some()
                    || !self.connected
                    || peer.run() != binding.run()
                    || peer.process().is_some()
                {
                    return Err(
                        "Authenticated coordinator differs from actual child bootstrap".into()
                    );
                }
                self.peer = Some(peer);
                self.exchange = Exchange::Welcome;
            }
            LifecyclePipeObservation::ReadPending { operation } => {
                PendingRead::validate(self.read.as_ref(), operation)?;
                if self.exchange == Exchange::Welcome {
                    self.hello_pending = true;
                }
            }
            LifecyclePipeObservation::WriteIssued { operation, at: _ } => {
                PendingWrite::issued(self.write.as_mut(), operation)?;
            }
            LifecyclePipeObservation::Written { operation, bytes } => {
                PendingWrite::written(self.write.as_ref(), operation, bytes)?;
                self.write = None;
            }
            LifecyclePipeObservation::WriteRevoked { operation: _, buffer } => {
                drop(buffer);
                return Err("Production lifecycle observed an unrequested write revocation".into());
            }
            LifecyclePipeObservation::Received { operation, frame } => {
                PendingRead::validate(self.read.as_ref(), operation)?;
                self.read = None;
                if !self.admission.open() {
                    drop(frame);
                    return Ok(());
                }
                match self.exchange {
                    Exchange::Welcome => {
                        self.handshake
                            .as_mut()
                            .ok_or("Original lifecycle binding missing")?
                            .welcome(&FrameCodec::decode(frame.as_bytes())?)?;
                        self.exchange = Exchange::Register;
                    }
                    Exchange::Register => {
                        self.handshake
                            .as_mut()
                            .ok_or("Original lifecycle binding missing")?
                            .registered(&FrameCodec::decode(frame.as_bytes())?)?;
                        self.requests = Some(ClientRequests::new(
                            self.handshake.as_ref().ok_or("Original registered binding missing")?,
                        )?);
                        self.ui = Some(UiChallengeExchange::new(
                            self.handshake.as_ref().ok_or("Original registered binding missing")?,
                        )?);
                        self.exchange = Exchange::Application;
                    }
                    Exchange::Application => {
                        if self.received.is_some() {
                            return Err("Original lifecycle frame already retained".into());
                        }
                        match self.handle_application(&frame) {
                            Ok(true) => self.received = Some(frame),
                            Ok(false) => drop(frame),
                            Err(cause) => {
                                self.received = Some(frame);
                                return Err(cause);
                            }
                        }
                    }
                    Exchange::Preparing => {
                        return Err("Lifecycle message arrived before native preparation".into());
                    }
                }
            }
        }
        Ok(())
    }
}
