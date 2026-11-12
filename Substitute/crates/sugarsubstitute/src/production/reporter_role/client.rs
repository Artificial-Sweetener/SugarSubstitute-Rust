// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adopt only the actual inherited Reporter bootstrap and authenticate its original parent.
use std::num::NonZeroUsize;
use sugarsubstitute_contracts::{execution::*, incident::*};
use sugarsubstitute_execution::{
    ExecutionClient, PurposePipeResources, purpose_pipe::*, windows_purpose_pipe::*,
};
pub(in crate::production) struct ReporterClient {
    input: Option<InheritedLifecycleBootstrap>,
    original: LifecyclePipeDeadline,
    session: Option<PurposePipeSession>,
    peer: Option<VerifiedPurposePeer>,
    key: Option<DiagnosticKey>,
    capability: Option<LifecycleClientCapability>,
    admission: Option<PurposePipeAdmissionRejection>,
    first: Option<LifecyclePipeError>,
    next_operation: u64,
    hello_issued: bool,
    hello_written: bool,
    write_buffer: Option<LifecyclePipeWriteBuffer>,
    received: Option<ReceivedLifecycleFrame>,
    terminal: Option<JobCompletion<PurposePipeReport, LifecyclePipeFailure>>,
    written: Option<LifecyclePipeOperationId>,
}
/// Same-session native facts and complete original input; wire decoding happens on Core.
pub(super) struct NativeInput {
    pub(super) peer: VerifiedPurposePeer,
    pub(super) key: DiagnosticKey,
    pub(super) frame: ReceivedLifecycleFrame,
}
impl ReporterClient {
    /// Own lexical input; native role/parent validation happens in the pre-admitted worker.
    pub(in crate::production) fn new(
        input: InheritedLifecycleBootstrap,
        original: LifecyclePipeDeadline,
    ) -> Self {
        Self {
            input: Some(input),
            original,
            session: None,
            peer: None,
            key: None,
            capability: None,
            admission: None,
            first: None,
            next_operation: 0,
            hello_issued: false,
            hello_written: false,
            write_buffer: None,
            received: None,
            terminal: None,
            written: None,
        }
    }
    /// Reserve the actual persistent native job before transferring its inherited section.
    /// Denial keeps the input and real admission cause; no numeric role selects native privilege.
    pub(in crate::production) fn reserve(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
    ) -> Result<(), LifecyclePipeError> {
        if self.session.is_some() || self.admission.is_some() {
            return Err(LifecyclePipeError::Busy);
        }
        let Some(input) = self.input.take() else {
            return Err(LifecyclePipeError::AlreadyTransferred);
        };
        let permit = match client.reserve_purpose_pipe_client(
            scope,
            spec,
            Purpose::Diagnostic(DiagnosticRole::Reporter),
            self.original,
            PurposePipeResources::Standard,
        ) {
            Ok(value) => value,
            Err(cause) => {
                self.input = Some(input);
                self.admission = Some(cause);
                return Err(LifecyclePipeError::NotReady);
            }
        };
        self.session = permit.bind(input);
        if self.session.is_none() {
            self.first = Some(LifecyclePipeError::Closed);
            return Err(LifecyclePipeError::Closed);
        }
        Ok(())
    }
    fn next(&mut self) -> Result<LifecyclePipeOperationId, LifecyclePipeError> {
        self.next_operation =
            self.next_operation.checked_add(1).ok_or(LifecyclePipeError::InvalidOperation)?;
        LifecyclePipeOperationId::new(self.next_operation)
    }
    /// Drain actual facts only; expected metadata and queued validation never count as authentication.
    pub(in crate::production) fn poll(&mut self) -> Result<bool, LifecyclePipeError> {
        let mut changed = false;
        for _ in 0..32 {
            if self.received.is_some() {
                break;
            }
            let session = self.session.as_mut().ok_or(LifecyclePipeError::NotReady)?;
            let Some(fact) = session.drain(NonZeroUsize::MIN).into_iter().next() else { break };
            changed = true;
            match fact {
                PurposePipeObservation::Prepared => {
                    let key = session
                        .control()
                        .prepared_key()
                        .and_then(PurposeKey::diagnostic)
                        .ok_or(LifecyclePipeError::NotReady)?;
                    if key.role() != DiagnosticRole::Reporter || self.key.is_some() {
                        return Err(LifecyclePipeError::InvalidMessage);
                    }
                    self.key = Some(key);
                    self.capability = session.control().take_client_capability()?;
                }
                PurposePipeObservation::Connected => {
                    let capability =
                        self.capability.as_ref().ok_or(LifecyclePipeError::NotReady)?;
                    session.control().verify_capability(
                        *capability.bytes(),
                        session.control().original_deadline(),
                    )?;
                }
                PurposePipeObservation::Authenticated(peer) => {
                    if Some(peer.key()) != self.key.map(PurposeKey::Diagnostic)
                        || peer.process().is_some()
                        || self.peer.is_some()
                    {
                        return Err(LifecyclePipeError::InvalidMessage);
                    }
                    self.peer = Some(peer);
                }
                PurposePipeObservation::Received { frame, .. } => self.received = Some(frame),
                PurposePipeObservation::Written { operation, .. } => {
                    self.written = Some(operation);
                    if self.hello_issued {
                        self.hello_written = true;
                        self.capability = None;
                    }
                }
                PurposePipeObservation::WriteRevoked { buffer, .. } => {
                    self.write_buffer = Some(buffer)
                }
                PurposePipeObservation::ReadPending { .. }
                | PurposePipeObservation::WriteIssued { .. } => {}
            }
        }
        if self.peer.is_some() && !self.hello_issued {
            self.send_capability()?;
        }
        if self.terminal.is_none() {
            self.terminal = self.session.as_mut().and_then(PurposePipeSession::take_terminal);
        }
        Ok(changed)
    }
    fn send_capability(&mut self) -> Result<(), LifecyclePipeError> {
        let operation = self.next()?;
        let session = self.session.as_ref().ok_or(LifecyclePipeError::NotReady)?;
        if self.write_buffer.is_none() {
            self.write_buffer = session.control().take_write_buffer()?;
        }
        let mut buffer = self.write_buffer.take().ok_or(LifecyclePipeError::Busy)?;
        let capability = self.capability.as_ref().ok_or(LifecyclePipeError::NotReady)?;
        buffer.as_mut_bytes()[..32].copy_from_slice(capability.bytes());
        match session.control().write(operation, session.control().original_deadline(), buffer, 32)
        {
            Ok(_) => {
                self.hello_issued = true;
                let read = self.next()?;
                self.session.as_ref().ok_or(LifecyclePipeError::NotReady)?.control().read(
                    read,
                    self.session
                        .as_ref()
                        .ok_or(LifecyclePipeError::NotReady)?
                        .control()
                        .original_deadline(),
                )?;
                Ok(())
            }
            Err(rejected) => {
                self.first.get_or_insert(rejected.error);
                self.write_buffer = Some(rejected.buffer);
                Err(rejected.error)
            }
        }
    }
    /// Transfer no decoded values on the native thread; retain the exact session association.
    pub(super) fn take_native_input(&mut self) -> Option<NativeInput> {
        let peer = self.peer?;
        let key = self.key?;
        let frame = self.received.take()?;
        Some(NativeInput { peer, key, frame })
    }
    /// Reissue only a passive authenticated read after the previous input loan was consumed.
    pub(super) fn continue_read(&mut self) -> Result<(), LifecyclePipeError> {
        if self.peer.is_none() || self.received.is_some() {
            return Err(LifecyclePipeError::Busy);
        }
        let operation = self.next()?;
        self.session.as_ref().ok_or(LifecyclePipeError::NotReady)?.control().read_passive(operation)
    }
    pub(in crate::production) fn close(&self) {
        if let Some(session) = &self.session {
            session.control().close();
        }
    }
    /// Borrow only the session's original charged encoding destination.
    pub(super) fn take_output(
        &mut self,
    ) -> Result<Option<LifecyclePipeWriteBuffer>, LifecyclePipeError> {
        self.session.as_ref().ok_or(LifecyclePipeError::NotReady)?.control().take_write_buffer()
    }
    /// Return actual authenticated session matching facts; no wire/UI datum selects this key.
    pub(super) fn key(&self) -> Option<DiagnosticKey> {
        self.peer.and_then(|peer| peer.key().diagnostic())
    }
    /// Submit already encoded complete bytes; every rejection retains the identical original loan.
    pub(super) fn write_encoded(
        &mut self,
        until: Moment,
        buffer: LifecyclePipeWriteBuffer,
        length: usize,
    ) -> Result<LifecyclePipeOperationId, LifecyclePipeWriteRejection> {
        let operation = match self.next() {
            Ok(value) => value,
            Err(error) => return Err(LifecyclePipeWriteRejection { error, buffer }),
        };
        let Some(session) = &self.session else {
            return Err(LifecyclePipeWriteRejection {
                error: LifecyclePipeError::NotReady,
                buffer,
            });
        };
        session.control().write(operation, until, buffer, length)?;
        Ok(operation)
    }
    pub(super) fn take_written(&mut self) -> Option<LifecyclePipeOperationId> {
        self.written.take()
    }
    pub(super) fn terminal_delivered(&self) -> bool {
        self.terminal.is_some() || self.session.is_none()
    }
    pub(super) fn original_until(&self) -> Option<Moment> {
        self.session.as_ref().map(|session| session.control().original_deadline())
    }
    pub(super) fn failure(&self) -> Option<&(dyn std::error::Error + 'static)> {
        if let Some(JobOutcome::Failure(cause)) = self.terminal.as_ref().map(|done| &done.outcome) {
            return Some(cause);
        }
        if let Some(cause) = &self.admission {
            return Some(&cause.error);
        }
        self.first.as_ref().map(|cause| cause as _)
    }
}
