// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe actual pre-capture transport facts without acquiring or manufacturing authority.
use super::*;
/// Last native handshake predicates retained by this actual channel owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production::child) enum HandshakePhase {
    Unadmitted,
    Admitted,
    Prepared,
    Connected,
    Authenticated,
}
/// Outcome of the actual captured-pair poll, distinct from a later handshake observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production::child) enum CaptureInput {
    Unchecked,
    NoPeer,
    Absent,
    Transferred,
    Closed,
}
impl ChildChannel {
    /// Borrow achieved native facts only; closure does not erase the last authenticated phase.
    pub(in crate::production::child) fn handshake_phase(&self) -> HandshakePhase {
        if self.session.is_none() {
            HandshakePhase::Unadmitted
        } else if self.peer.is_some() {
            HandshakePhase::Authenticated
        } else if self.connected {
            HandshakePhase::Connected
        } else if self.handshake.is_some() {
            HandshakePhase::Prepared
        } else {
            HandshakePhase::Admitted
        }
    }
    /// Borrow the last actual captured-pair polling outcome; this does not inspect native state.
    pub(in crate::production::child) const fn capture_input(&self) -> CaptureInput {
        self.capture_input
    }
    /// Move the actual authenticated pair once while preserving the precise inert poll outcome.
    /// No copied peer facts, diagnostic phase or inherited token can construct this pair.
    pub(in crate::production::child) fn take_captured_startup(
        &mut self,
    ) -> Option<CapturedStartup> {
        if !self.admission.open() {
            self.capture_input = CaptureInput::Closed;
            return None;
        }
        if self.peer.is_none() {
            self.capture_input = CaptureInput::NoPeer;
            return None;
        }
        let input = self.session.as_ref()?.control.take_captured_startup();
        self.capture_input =
            if input.is_some() { CaptureInput::Transferred } else { CaptureInput::Absent };
        input
    }
}
