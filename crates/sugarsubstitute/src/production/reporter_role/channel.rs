// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep every actual frame until the same session's native authentication and guarded dispatch.
use std::num::NonZeroUsize;
use sugarsubstitute_contracts::{execution::*, incident::*, lifecycle::ProcessKey};
use sugarsubstitute_crash::diagnostic_client::*;
use sugarsubstitute_execution::diagnostic_pipe::*;
/// No matching header, connected observation or queued verification admits a command.
pub(super) struct Channel {
    pub(super) session: DiagnosticPipeSession,
    key: DiagnosticKey,
    process: Option<ProcessKey>,
    peer: Option<VerifiedDiagnosticPeer>,
    authentication: Option<ReceivedLifecycleFrame>,
    received: Option<ReceivedLifecycleFrame>,
    rejected: Option<ReceivedDiagnosticRequest>,
    read_pending: bool,
    next_operation: u64,
    verification_queued: bool,
    pub(super) first: Option<LifecyclePipeError>,
    pub(super) wire: Option<DiagnosticWireError>,
    pub(super) terminal: Option<JobCompletion<DiagnosticPipeReport, LifecyclePipeFailure>>,
    pub(super) written: Option<LifecyclePipeOperationId>,
    router: DiagnosticRequestRouter,
}
impl Channel {
    pub(super) const fn key(&self) -> DiagnosticKey {
        self.key
    }

    pub(super) fn new(session: DiagnosticPipeSession, key: DiagnosticKey) -> Self {
        Self {
            session,
            key,
            process: None,
            peer: None,
            authentication: None,
            received: None,
            rejected: None,
            read_pending: false,
            next_operation: 0,
            verification_queued: false,
            first: None,
            wire: None,
            terminal: None,
            written: None,
            router: DiagnosticRequestRouter::new(key),
        }
    }
    pub(super) fn bind(&mut self, process: ProcessKey) -> Result<(), LifecyclePipeError> {
        if self.process.is_some_and(|old| old != process) {
            return Err(LifecyclePipeError::InvalidOperation);
        }
        self.process = Some(process);
        Ok(())
    }
    pub(super) fn next(&mut self) -> Result<LifecyclePipeOperationId, LifecyclePipeError> {
        let next =
            self.next_operation.checked_add(1).ok_or(LifecyclePipeError::InvalidOperation)?;
        let operation = LifecyclePipeOperationId::new(next)?;
        self.next_operation = next;
        Ok(operation)
    }
    pub(super) fn poll(&mut self) -> Result<bool, LifecyclePipeError> {
        let mut changed = false;
        // One receive is issued at a time; never drain another frame into an occupied cell.
        for _ in 0..32 {
            if self.received.is_some() || self.rejected.is_some() {
                break;
            }
            let Some(observation) = self.session.drain(NonZeroUsize::MIN).into_iter().next() else {
                break;
            };
            changed = true;
            match observation {
                DiagnosticPipeObservation::Prepared => {}
                DiagnosticPipeObservation::Connected => {
                    if self.process.is_none() {
                        self.first.get_or_insert(LifecyclePipeError::NotReady);
                        return Err(LifecyclePipeError::NotReady);
                    }
                    self.issue_read(false)?;
                }
                DiagnosticPipeObservation::Authenticated(peer) => {
                    if peer.key() != self.key
                        || peer.process() != self.process
                        || self.process.is_none()
                        || !self.verification_queued
                        || self.peer.is_some_and(|old| old != peer)
                    {
                        self.first.get_or_insert(LifecyclePipeError::InvalidMessage);
                        return Err(LifecyclePipeError::InvalidMessage);
                    }
                    self.peer = Some(peer);
                    drop(self.authentication.take());
                    self.issue_read(true)?;
                }
                DiagnosticPipeObservation::Received { frame, .. } => {
                    self.read_pending = false;
                    if self.peer.is_some() {
                        self.received = Some(frame);
                    } else {
                        self.authentication = Some(frame);
                        let raw = self
                            .authentication
                            .as_ref()
                            .ok_or(LifecyclePipeError::InvalidMessage)?;
                        let candidate = capability(raw.as_bytes())?;
                        self.session.control().verify_capability(
                            candidate,
                            self.session.control().original_deadline(),
                        )?;
                        self.verification_queued = true;
                    }
                }
                DiagnosticPipeObservation::ReadPending { .. } => {}
                DiagnosticPipeObservation::WriteRevoked { buffer, .. } => {
                    drop(buffer);
                }
                DiagnosticPipeObservation::WriteIssued { .. } => {}
                DiagnosticPipeObservation::Written { operation, .. } => {
                    self.written = Some(operation)
                }
            }
        }
        if self.terminal.is_none() {
            self.terminal = self.session.take_terminal();
        }
        Ok(changed)
    }
    fn issue_read(&mut self, passive: bool) -> Result<(), LifecyclePipeError> {
        if self.read_pending || self.received.is_some() || self.rejected.is_some() {
            return Err(LifecyclePipeError::Busy);
        }
        let operation = self.next()?;
        let admitted = if passive {
            self.session.control().read_passive(operation)
        } else {
            self.session.control().read(operation, self.session.control().original_deadline())
        };
        if let Err(cause) = admitted {
            self.first.get_or_insert(cause);
            return Err(cause);
        }
        self.read_pending = true;
        Ok(())
    }
    /// Parsing occurs only in the caller's finite CPU/blocking worker, retaining this channel loan.
    pub(super) fn receive(
        &mut self,
    ) -> Result<Option<(ReceivedDiagnosticRequest, DiagnosticAdmission)>, DiagnosticWireError> {
        let Some(peer) = self.peer else { return Ok(None) };
        let Some(frame) = self.received.take() else { return Ok(None) };
        let request = match ReceivedDiagnosticRequest::from_native(peer, self.key, frame) {
            Ok(value) => value,
            Err(rejected) => {
                self.received = Some(rejected.frame);
                self.wire.get_or_insert(rejected.error);
                return Err(rejected.error);
            }
        };
        let admission = match self.router.admit(request.bytes()) {
            Ok(value) => value,
            Err(cause) => {
                self.wire.get_or_insert(cause);
                self.rejected = Some(request);
                return Err(cause);
            }
        };
        Ok(Some((request, admission)))
    }
    pub(super) fn continue_read(&mut self) -> Result<(), LifecyclePipeError> {
        self.issue_read(true)
    }
    /// Complete only the same issued request; rejection preserves its caller-owned token.
    pub(super) fn complete(
        &mut self,
        issued: &sugarsubstitute_crash::diagnostic_client::IssuedDiagnosticRequest,
        reply: sugarsubstitute_crash::diagnostic_client::DiagnosticReply,
    ) -> Result<(), DiagnosticWireError> {
        self.router.complete(issued, reply).inspect_err(|cause| {
            self.wire.get_or_insert(*cause);
        })
    }
    pub(super) fn authenticated(&self) -> bool {
        self.peer.is_some()
    }
    pub(super) fn close(&self) {
        self.session.control().close();
    }
    pub(super) fn closed_inputs_returned(&self) -> bool {
        self.authentication.is_none() && self.received.is_none() && self.rejected.is_none()
    }
    /// Return completed incoming loans only after the composing parent proves actual family closure.
    /// All original wire/native errors and completion funding remain in this same channel.
    pub(super) fn return_closed_incoming(&mut self) {
        drop(self.authentication.take());
        drop(self.received.take());
        drop(self.rejected.take());
    }
}
fn capability(bytes: &[u8]) -> Result<[u8; 32], LifecyclePipeError> {
    bytes.try_into().map_err(|_| LifecyclePipeError::InvalidMessage)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capability_prelude_requires_exact_bytes_without_admitting_a_command() {
        assert_eq!(capability(&[7; 32]), Ok([7; 32]));
        assert_eq!(capability(&[7; 31]), Err(LifecyclePipeError::InvalidMessage));
        assert_eq!(capability(&[7; 33]), Err(LifecyclePipeError::InvalidMessage));
    }
}
