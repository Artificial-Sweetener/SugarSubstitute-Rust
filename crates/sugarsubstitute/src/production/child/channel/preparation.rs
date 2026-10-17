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
    /// Borrow only the original scope/job-matched terminal metadata, even before result transfer.
    /// Explicit closure alone leaves this absent; no metadata grants native retirement authority.
    pub(in crate::production::child) const fn terminal_kind(
        &self,
    ) -> Option<sugarsubstitute_contracts::execution::TerminalKind> {
        self.admission.terminal()
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

#[cfg(test)]
mod tests {
    use super::*;
    use sugarsubstitute_contracts::{
        execution::TerminalKind,
        lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration},
    };

    #[test]
    fn retained_terminal_metadata_remains_visible_without_typed_completion() -> Result<()> {
        let run = RunKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([1; 32]),
                epoch: CoordinatorEpoch::new([2; 16])?,
            },
            instance: InstanceId::new(1)?,
            generation: RunGeneration::new(1)?,
        };
        for kind in [
            TerminalKind::Success,
            TerminalKind::Failure,
            TerminalKind::Panicked,
            TerminalKind::Cancelled,
            TerminalKind::TimedOut,
            TerminalKind::Coalesced,
        ] {
            let mut channel = ChildChannel::new(
                InheritedLifecycleBootstrap::new(1)?,
                LifecyclePipeDeadline::new(100, 100)?,
                run,
            );
            assert_eq!(channel.terminal_kind(), None);
            channel.request_close();
            assert_eq!(channel.terminal_kind(), None);
            // Exercise only retained metadata; no transport, peer or typed result is fabricated.
            channel.admission.observe_terminal(kind)?;
            assert_eq!(channel.terminal_kind(), Some(kind));
            assert!(channel.terminal().is_none());
            assert!(channel.take_captured_startup().is_none());
            assert_eq!(channel.capture_input(), CaptureInput::Closed);
            assert_eq!(channel.terminal_kind(), Some(kind));
            channel.admission.observe_terminal(kind)?;
            assert_eq!(channel.terminal_kind(), Some(kind));
        }
        Ok(())
    }
}
