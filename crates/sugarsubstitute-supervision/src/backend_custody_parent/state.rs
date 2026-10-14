// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own the authenticated fixed-frame transaction and correlation state.
use super::*;
pub(super) struct State {
    pub(super) prepared: bool,
    pub(super) session: BackendCustodyPipeSession,
    pub(super) peer: Option<VerifiedBackendCustodyPeer>,
    pub(super) read: Option<LifecyclePipeOperationId>,
    pub(super) write: Option<LifecyclePipeOperationId>,
    pub(super) latest: u64,
    pub(super) nonce: u64,
    pub(super) pending: Option<(BackendCustodyFrame, Moment)>,
    pub(super) receipt: Option<ReceiptFacts>,
    pub(super) failure: Option<NativeObservationError>,
    pub(super) closing: bool,
    pub(super) established: bool,
    pub(super) registrations: [Option<retirement::Registration>; 64],
    pub(super) finish: Option<finish::Finish>,
}
impl State {
    /// Allocate no messages while advancing the sole connection-local operation sequence.
    fn operation(&mut self) -> Result<LifecyclePipeOperationId, NativeObservationError> {
        self.latest =
            self.latest.checked_add(1).ok_or_else(|| reject("custody_parent_operation"))?;
        LifecyclePipeOperationId::new(self.latest)
            .map_err(|cause| NativeObservationError::new("custody_parent_operation", cause))
    }
    /// Correlate every sealed actual fact before moving its original command/receipt.
    pub(super) fn consume(
        &mut self,
        max: NonZeroUsize,
        key: BackendCustodyKey,
    ) -> Result<(), NativeObservationError> {
        for fact in self.session.drain(max) {
            match fact {
                BackendCustodyPipeObservation::Prepared => {
                    self.prepared = true;
                }
                BackendCustodyPipeObservation::Connected => {
                    if self.read.is_some() || self.peer.is_some() {
                        return Err(reject("custody_parent_connected"));
                    }
                    let op = self.operation()?;
                    self.session
                        .control()
                        .read(op, self.session.control().original_deadline())
                        .map_err(|cause| {
                            NativeObservationError::new("custody_parent_hello_read", cause)
                        })?;
                    self.read = Some(op);
                }
                BackendCustodyPipeObservation::Authenticated(peer) => {
                    if self.peer.is_some() || peer.key() != key || peer.process().is_none() {
                        return Err(reject("custody_parent_authenticated"));
                    }
                    self.peer = Some(peer);
                }
                BackendCustodyPipeObservation::ReadPending { operation }
                    if self.read == Some(operation) => {}
                BackendCustodyPipeObservation::WriteIssued { operation, .. }
                    if self.write == Some(operation) => {}
                BackendCustodyPipeObservation::Written { operation, bytes } => {
                    correlate_written(self.write, operation, bytes)?;
                    if self.pending.is_some_and(|(frame, _)| {
                        matches!(frame.command, BackendCustodyCommand::Finish { .. })
                    }) {
                        tracing::info!(operation="backend_custody_finish_written", key=?key,
                            nonce=self.finish.as_ref().and_then(finish::Finish::nonce), native_operation=?operation, bytes,
                            "Original Finish native write completed");
                    }
                    self.write = None;
                }
                BackendCustodyPipeObservation::Received { operation, frame } => {
                    let value = decode_received(self.read, operation, key, frame.as_bytes())?;
                    if self.peer.is_none() {
                        let BackendCustodyCommand::Hello { capability } = value.command else {
                            return Err(reject("custody_parent_hello"));
                        };
                        if value.nonce != 1 {
                            return Err(reject("custody_parent_hello_nonce"));
                        }
                        self.session
                            .control()
                            .verify_capability(
                                capability,
                                self.session.control().original_deadline(),
                            )
                            .map_err(|cause| {
                                NativeObservationError::new("custody_parent_capability", cause)
                            })?;
                    } else {
                        let facts = correlate_reply(self.pending.map(|pending| pending.0), value)?;
                        let receipt = facts.receipt;
                        if receipt == BackendCustodyReceipt::Finished {
                            tracing::info!(operation="backend_custody_finish_received", key=?key, nonce=facts.nonce,
                                native_operation=?operation, write_pending=self.write.is_some(),
                                "Original correlated Finished response received");
                        }
                        self.receipt = Some(facts);
                        if let BackendCustodyReceipt::Prepared(resource) = receipt {
                            let slot = self
                                .registrations
                                .iter_mut()
                                .find(|slot| slot.is_none_or(|entry| entry.released.is_some()))
                                .ok_or_else(|| reject("custody_parent_registration_capacity"))?;
                            *slot = Some(retirement::Registration {
                                resource,
                                stop: None,
                                released: None,
                            });
                        }
                        if receipt == BackendCustodyReceipt::Established {
                            self.established = true;
                        }
                        self.pending = None;
                    }
                    self.read = None;
                    drop(frame);
                }
                BackendCustodyPipeObservation::WriteRevoked { buffer, .. } => {
                    drop(buffer);
                    return Err(reject("custody_parent_write_revoked"));
                }
                _ => return Err(reject("custody_parent_observation")),
            }
        }
        self.advance()
    }
    /// Retain and diagnose only the first actual transaction failure.
    pub(super) fn retain_failure(
        &mut self,
        key: BackendCustodyKey,
        cause: &NativeObservationError,
    ) {
        if self.failure.is_none() {
            tracing::error!(operation="backend_custody_parent_failed", key=?key, stage=cause.stage(),
                finish_nonce=self.finish.as_ref().and_then(finish::Finish::nonce),
                pending_nonce=self.pending.map(|(frame, _)| frame.nonce),
                pending_kind=self.pending.map(|(frame, _)| command_kind(&frame.command)),
                original_until=?self.pending.map(|(_, until)| until),
                read_operation=?self.read, write_operation=?self.write,
                receipt_nonce=self.receipt.map(|facts| facts.nonce),
                "First original custody transaction failure retained");
            self.failure = Some(cause.clone());
        }
    }
    /// Issue the original fixed message and one correlated bounded response receive.
    pub(super) fn advance(&mut self) -> Result<(), NativeObservationError> {
        if self.closing {
            return Ok(());
        }
        if self.write.is_none()
            && self.read.is_none()
            && let Some((frame, until)) = self.pending
            && let Some(mut buffer) = self
                .session
                .control()
                .take_write_buffer()
                .map_err(|cause| NativeObservationError::new("custody_parent_buffer", cause))?
        {
            let bytes = frame
                .encode()
                .map_err(|cause| NativeObservationError::new("custody_parent_encode", cause))?;
            buffer
                .as_mut_bytes()
                .get_mut(..BACKEND_CUSTODY_FRAME_BYTES)
                .ok_or_else(|| reject("custody_parent_capacity"))?
                .copy_from_slice(&bytes);
            let write = self.operation()?;
            self.session
                .control()
                .write(write, until, buffer, BACKEND_CUSTODY_FRAME_BYTES)
                .map_err(|cause| {
                    NativeObservationError::new("custody_parent_write_admission", cause.error)
                })?;
            self.write = Some(write);
            let read = self.operation()?;
            self.session.control().read(read, until).map_err(|cause| {
                NativeObservationError::new("custody_parent_read_admission", cause)
            })?;
            self.read = Some(read);
        }
        Ok(())
    }
}
/// Validate the original native read operation before decoding its bounded external bytes.
pub(super) fn decode_received(
    expected: Option<LifecyclePipeOperationId>,
    operation: LifecyclePipeOperationId,
    key: BackendCustodyKey,
    bytes: &[u8],
) -> Result<BackendCustodyFrame, NativeObservationError> {
    if expected != Some(operation) {
        return Err(reject("custody_parent_read"));
    }
    let value = BackendCustodyFrame::decode(bytes)
        .map_err(|cause| NativeObservationError::new("custody_parent_decode", cause))?;
    if value.key != key {
        return Err(reject("custody_parent_key"));
    }
    Ok(value)
}
/// Correlate the response to the actual original command, without manufacturing a peer.
pub(super) fn correlate_reply(
    pending: Option<BackendCustodyFrame>,
    value: BackendCustodyFrame,
) -> Result<ReceiptFacts, NativeObservationError> {
    let pending = pending.ok_or_else(|| reject("custody_parent_unsolicited"))?;
    if value.key != pending.key {
        return Err(reject("custody_parent_key"));
    }
    if value.nonce != pending.nonce {
        return Err(reject("custody_parent_nonce"));
    }
    let BackendCustodyCommand::Receipt(receipt) = value.command else {
        return Err(reject("custody_parent_receipt"));
    };
    if !matches_reply(pending.command, receipt) {
        return Err(reject("custody_parent_receipt_association"));
    }
    Ok(ReceiptFacts { key: value.key, nonce: value.nonce, receipt })
}
/// Accept only the actual queued operation's complete fixed-frame native write.
pub(super) fn correlate_written(
    expected: Option<LifecyclePipeOperationId>,
    operation: LifecyclePipeOperationId,
    bytes: u32,
) -> Result<(), NativeObservationError> {
    if expected != Some(operation) || bytes as usize != BACKEND_CUSTODY_FRAME_BYTES {
        return Err(reject("custody_parent_write"));
    }
    Ok(())
}
/// Match only the acknowledged original operation; a Success report cannot synthesize this.
fn matches_reply(command: BackendCustodyCommand, receipt: BackendCustodyReceipt) -> bool {
    match (command, receipt) {
        (BackendCustodyCommand::Establish { .. }, BackendCustodyReceipt::Established)
        | (BackendCustodyCommand::Finish { .. }, BackendCustodyReceipt::Finished) => true,
        (
            BackendCustodyCommand::Prepare { resource, .. },
            BackendCustodyReceipt::Prepared(actual),
        )
        | (BackendCustodyCommand::Root { resource, .. }, BackendCustodyReceipt::Root(actual))
        | (BackendCustodyCommand::Stop { resource, .. }, BackendCustodyReceipt::Released(actual)) => {
            resource == actual
        }
        _ => false,
    }
}
/// Return a bounded typed protocol rejection without paths or capability contents.
pub(super) fn reject(stage: &'static str) -> NativeObservationError {
    NativeObservationError::new(stage, LifecyclePipeError::InvalidMessage)
}

/// Classify the retained operation without exposing payloads, paths or capabilities.
fn command_kind(command: &BackendCustodyCommand) -> &'static str {
    match command {
        BackendCustodyCommand::Hello { .. } => "Hello",
        BackendCustodyCommand::Establish { .. } => "Establish",
        BackendCustodyCommand::Prepare { .. } => "Prepare",
        BackendCustodyCommand::Root { .. } => "Root",
        BackendCustodyCommand::Stop { .. } => "Stop",
        BackendCustodyCommand::Finish { .. } => "Finish",
        BackendCustodyCommand::Receipt(_) => "Receipt",
    }
}
