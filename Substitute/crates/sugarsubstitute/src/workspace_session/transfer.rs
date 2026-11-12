// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Assemble one bounded ordered UTF-8 capture while retaining its original memory admission.
use super::*;
use sha2::{Digest, Sha256};
use sugarsubstitute_application::workspace_session::CaptureTicket;
use sugarsubstitute_contracts::{
    desktop_session::{MAX_CHUNK_BYTES, decode_checkpoint, wire::SessionTransfer},
    execution::{MemoryClaim, MemoryLease, Moment},
};

/// Refusal of an untrusted transfer; none of these outcomes acknowledge durable state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransferError {
    /// Transfer metadata/UTF-8 byte total does not match the active host capture.
    Invalid,
    /// A chunk is repeated, skipped, reordered or belongs to another transfer.
    Order,
    /// The original finite transfer deadline expired.
    Deadline,
    /// The admitted byte/digest identity differs from the completed transfer.
    Integrity,
    /// The current owner cannot fund the complete announced transfer and decode.
    Memory,
    /// Shared checkpoint shape/membership validation failed.
    Contract(SessionError),
}
impl fmt::Display for TransferError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Workspace transfer refused: {self:?}")
    }
}
impl Error for TransferError {}

/// Admitted immutable captured checkpoint with the charge funding its decoded state.
pub struct CapturedWorkspace {
    /// Decoded, shape-validated DTO; application admission still checks the complete fence.
    pub checkpoint: SessionCheckpoint,
    _memory: MemoryClaim,
}
/// One exact host-requested capture. Dropping it cancels assembly without detached work.
pub struct CaptureTransfer {
    metadata: SessionTransfer,
    deadline: Moment,
    scope: SessionScope,
    bytes: Vec<u8>,
    index: u64,
    hash: Sha256,
    memory: MemoryClaim,
}
impl CaptureTransfer {
    /// Admit bounded metadata before reserving bytes. Caller first authenticates the message
    /// and matches its request ID to the active host capture. No page data grants admission.
    /// # Errors
    /// Refuses foreign capture identity/revision/direction, impossible bounds or memory failure.
    pub fn new(
        metadata: SessionTransfer,
        ticket: &CaptureTicket,
        scope: SessionScope,
        deadline: Moment,
        now: Moment,
        memory: &MemoryLease,
    ) -> Result<Self, TransferError> {
        if !metadata.valid()
            || metadata.direction != "capture"
            || metadata.checkpoint_id != ticket.checkpoint.as_str()
            || metadata.revision != ticket.revision
            || metadata.bytes < metadata.chunks
            || metadata.bytes > metadata.chunks * MAX_CHUNK_BYTES as u64
            || metadata.digest.len() != 64
            || !metadata
                .digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(TransferError::Invalid);
        }
        if now >= deadline {
            return Err(TransferError::Deadline);
        }
        let claim = memory
            .claim(metadata.bytes.checked_mul(4).ok_or(TransferError::Memory)?)
            .map_err(|_| TransferError::Memory)?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(metadata.bytes as usize).map_err(|_| TransferError::Memory)?;
        Ok(Self { metadata, deadline, scope, bytes, index: 0, hash: Sha256::new(), memory: claim })
    }
    /// Append the exact next chunk. UTF-8 bytes, rather than UTF-16 JavaScript units, define
    /// all admission and digest bounds. A refusal leaves the transfer available for explicit retirement.
    /// # Errors
    /// Refuses late, foreign, out-of-order, empty, excessive or trailing chunk bytes.
    pub fn chunk(
        &mut self,
        transfer_id: &str,
        index: u64,
        text: &str,
        now: Moment,
    ) -> Result<(), TransferError> {
        if now >= self.deadline {
            return Err(TransferError::Deadline);
        }
        if transfer_id != self.metadata.id || index != self.index || index >= self.metadata.chunks {
            return Err(TransferError::Order);
        }
        if text.is_empty()
            || text.len() > MAX_CHUNK_BYTES
            || text.len() > (self.metadata.bytes as usize).saturating_sub(self.bytes.len())
        {
            return Err(TransferError::Invalid);
        }
        self.bytes.extend_from_slice(text.as_bytes());
        self.hash.update(text.as_bytes());
        self.index += 1;
        Ok(())
    }
    /// Complete one capture after exact count, length, digest, checkpoint and scope checks.
    /// The returned state keeps its memory charge through application/store processing.
    /// # Errors
    /// Refuses incomplete/corrupt/late bytes, malformed checkpoints or foreign namespace.
    pub fn finish(
        self,
        transfer_id: &str,
        now: Moment,
    ) -> Result<CapturedWorkspace, TransferError> {
        if now >= self.deadline {
            return Err(TransferError::Deadline);
        }
        if transfer_id != self.metadata.id || self.index != self.metadata.chunks {
            return Err(TransferError::Order);
        }
        if self.bytes.len() as u64 != self.metadata.bytes
            || files::hex(&self.hash.finalize()) != self.metadata.digest
        {
            return Err(TransferError::Integrity);
        }
        let checkpoint = decode_checkpoint(&self.bytes).map_err(TransferError::Contract)?;
        if checkpoint.id != self.metadata.checkpoint_id
            || checkpoint.revision != self.metadata.revision
            || checkpoint.scope != self.scope
        {
            return Err(TransferError::Invalid);
        }
        Ok(CapturedWorkspace { checkpoint, _memory: self.memory })
    }
}
