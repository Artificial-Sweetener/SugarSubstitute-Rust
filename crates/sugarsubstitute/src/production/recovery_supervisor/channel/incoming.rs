// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Match native observations before changing any transport or startup phase.
use super::*;
impl Channel {
    /// Receive only observations issued by the sole retained native session owner.
    pub(super) fn observe(&mut self, observation: RecoveryPipeObservation) -> Result<()> {
        match observation {
            RecoveryPipeObservation::Prepared => {
                if self.key.is_some() || self.connected {
                    return Err("Recovery bootstrap repeated".into());
                }
                self.key = Some(
                    self.session
                        .control()
                        .prepared_key()
                        .ok_or("Recovery bootstrap key missing")?,
                );
                self.capability = self.session.control().take_client_capability()?;
                if self.capability.is_none() {
                    return Err("Recovery Hello capability absent".into());
                }
            }
            RecoveryPipeObservation::Connected => {
                if self.connected || self.key.is_none() {
                    return Err("Recovery connection order rejected".into());
                }
                self.connected = true;
                let candidate =
                    *self.capability.as_ref().ok_or("Recovery capability absent")?.bytes();
                self.session.control().verify_capability(candidate, self.original_until())?;
            }
            RecoveryPipeObservation::Authenticated(peer) => {
                if !self.connected
                    || self.authenticated
                    || Some(peer.key()) != self.key
                    || peer.process().is_some()
                {
                    return Err("Recovery actual Root peer rejected".into());
                }
                self.authenticated = true;
            }
            RecoveryPipeObservation::ReadPending { operation } => {
                if self.read != Some(operation) {
                    return Err("Recovery pending read identity rejected".into());
                }
            }
            RecoveryPipeObservation::Received { operation, frame } => {
                if self.received.is_some() {
                    return Err("Recovery receive output repeated".into());
                }
                // Adopt the real guard before checking the correlation or its untrusted bytes.
                self.received = Some(frame);
                if self.read != Some(operation) || !self.authenticated {
                    return Err("Recovery received identity rejected".into());
                }
                self.read = None;
                let decoded = decode(
                    self.received.as_ref().ok_or("Recovery receive guard missing")?.as_bytes(),
                    RecoveryReceiver::Supervisor,
                )?;
                if Some(decoded.key) != self.key
                    || self.rx_sequence.checked_add(1) != Some(decoded.sequence)
                {
                    return Err("Recovery received grant or sequence rejected".into());
                }
                self.rx_sequence = decoded.sequence;
            }
            RecoveryPipeObservation::WriteIssued { operation, .. } => {
                let write = self.write.as_mut().ok_or("Recovery write issue absent")?;
                write.receipt.issued(operation)?;
            }
            RecoveryPipeObservation::Written { operation, bytes } => {
                let write = self.write.as_ref().ok_or("Recovery write completion absent")?;
                if self.sent.is_some() {
                    return Err("Recovery unconsumed native write completion".into());
                }
                write.receipt.written(operation, bytes, self.tx_sequence)?;
                self.tx_sequence = write.receipt.sequence;
                self.hello_written |= write.sent == Sent::Hello;
                self.sent = Some(write.sent);
                self.write = None;
            }
            RecoveryPipeObservation::WriteRevoked { operation, buffer } => {
                let write = self.write.as_mut().ok_or("Recovery original write absent")?;
                if write.returned.is_some() {
                    return Err("Recovery write buffer return repeated".into());
                }
                write.returned = Some(buffer);
                if !self.closing {
                    return Err("Recovery write returned before shutdown".into());
                }
                write.receipt.returned(operation)?;
                let returned = write.returned.as_mut().ok_or("Recovery returned buffer missing")?;
                let encoded = returned
                    .as_mut_bytes()
                    .get(..write.receipt.bytes)
                    .ok_or("Recovery returned write extent changed")?;
                let frame = decode(encoded, RecoveryReceiver::Coordinator)?;
                if Some(frame.key) != self.key || frame.sequence != write.receipt.sequence {
                    return Err("Recovery returned write identity changed".into());
                }
                // Return the checked original loan so native retirement can finish.
                self.write = None;
            }
        }
        Ok(())
    }
}
