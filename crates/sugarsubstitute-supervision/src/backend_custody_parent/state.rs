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
    pub(super) finish: Option<(Moment, Option<u64>, bool)>,
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
                    if self.write != Some(operation)
                        || bytes as usize != BACKEND_CUSTODY_FRAME_BYTES
                    {
                        return Err(reject("custody_parent_write"));
                    }
                    self.write = None;
                }
                BackendCustodyPipeObservation::Received { operation, frame } => {
                    if self.read != Some(operation) {
                        return Err(reject("custody_parent_read"));
                    }
                    let value = BackendCustodyFrame::decode(frame.as_bytes()).map_err(|cause| {
                        NativeObservationError::new("custody_parent_decode", cause)
                    })?;
                    if value.key != key {
                        return Err(reject("custody_parent_key"));
                    }
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
                        let (pending, _) =
                            self.pending.ok_or_else(|| reject("custody_parent_unsolicited"))?;
                        if value.nonce != pending.nonce {
                            return Err(reject("custody_parent_nonce"));
                        }
                        let BackendCustodyCommand::Receipt(receipt) = value.command else {
                            return Err(reject("custody_parent_receipt"));
                        };
                        if !matches_reply(pending.command, receipt) {
                            return Err(reject("custody_parent_receipt_association"));
                        }
                        self.receipt = Some(ReceiptFacts { key, nonce: value.nonce, receipt });
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
