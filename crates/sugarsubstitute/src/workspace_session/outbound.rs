// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Encode materialization on a storage worker and deliver bounded UTF-8 chunks lazily.
use super::*;
use sha2::{Digest, Sha256};
use sugarsubstitute_contracts::{
    desktop_session::{
        MAX_CHUNK_BYTES, encode_checkpoint,
        wire::{SessionFence, SessionOperation, SessionTransfer},
    },
    execution::{JobContext, MemoryClaim},
};
/// Validated encoded restore state, charged through the final native transport delivery.
pub struct EncodedMaterialization {
    bytes: String,
    chunks: Vec<(usize, usize)>,
    digest: String,
    checkpoint: String,
    revision: u64,
    _memory: MemoryClaim,
}
impl EncodedMaterialization {
    /// Encode a verified checkpoint only on an admitted finite worker; the UI pump only copies
    /// one bounded chunk at a time. The original decoded checkpoint remains separately charged.
    /// # Errors
    /// Refuses shared shape/bounds, cancellation, deadline and memory admission failures.
    pub fn encode(
        checkpoint: &SessionCheckpoint,
        context: &JobContext,
    ) -> Result<Self, StoreError> {
        check(context)?;
        let memory = context
            .memory()
            .claim(MAX_CHECKPOINT_BYTES as u64 * 2 + 128 * 1024)
            .map_err(io::Error::other)?;
        let bytes = String::from_utf8(encode_checkpoint(checkpoint)?).map_err(io::Error::other)?;
        let mut chunks = Vec::new();
        let mut start = 0;
        while start < bytes.len() {
            check(context)?;
            let mut end = start.saturating_add(MAX_CHUNK_BYTES).min(bytes.len());
            while !bytes.is_char_boundary(end) {
                end -= 1;
            }
            chunks.push((start, end));
            start = end;
        }
        if chunks.is_empty() || chunks.len() > 8192 {
            return Err(SessionError::TooLarge.into());
        }
        let digest = files::hex(&Sha256::digest(bytes.as_bytes()));
        check(context)?;
        Ok(Self {
            bytes,
            chunks,
            digest,
            checkpoint: checkpoint.id.clone(),
            revision: checkpoint.revision,
            _memory: memory,
        })
    }
}
pub(super) struct Outbound {
    request: String,
    transfer: String,
    encoded: EncodedMaterialization,
    next: usize,
}
impl Outbound {
    pub(super) fn new(request: String, transfer: String, encoded: EncodedMaterialization) -> Self {
        Self { request, transfer, encoded, next: 0 }
    }
    pub(super) fn next(&mut self, fence: &SessionFence) -> Option<(String, SessionOperation)> {
        let operation = match self.next {
            0 => SessionOperation::Begin {
                transfer: SessionTransfer {
                    id: self.transfer.clone(),
                    checkpoint_id: self.encoded.checkpoint.clone(),
                    revision: self.encoded.revision,
                    bytes: self.encoded.bytes.len() as u64,
                    chunks: self.encoded.chunks.len() as u64,
                    digest: self.encoded.digest.clone(),
                    direction: "materialize".into(),
                },
            },
            step if step <= self.encoded.chunks.len() => {
                let (start, end) = self.encoded.chunks[step - 1];
                SessionOperation::Chunk {
                    transfer_id: self.transfer.clone(),
                    index: (step - 1) as u64,
                    text: self.encoded.bytes[start..end].into(),
                }
            }
            step if step == self.encoded.chunks.len() + 1 => {
                SessionOperation::Commit { transfer_id: self.transfer.clone() }
            }
            step if step == self.encoded.chunks.len() + 2 => SessionOperation::Materialize {
                fence: fence.clone(),
                transfer_id: self.transfer.clone(),
            },
            _ => return None,
        };
        self.next += 1;
        Some((self.request.clone(), operation))
    }
}
/// Stop an encoding on cancellation, or on a cutoff when one was set. A restore encoding is
/// submitted without a cutoff of its own, so only the scope's shutdown cancellation ends it early.
fn check(context: &JobContext) -> Result<(), StoreError> {
    if context.cancellation_requested() {
        return Err(io::Error::from(io::ErrorKind::Interrupted).into());
    }
    if context.deadline().is_some_and(|deadline| context.now() >= deadline) {
        return Err(io::Error::from(io::ErrorKind::TimedOut).into());
    }
    Ok(())
}
