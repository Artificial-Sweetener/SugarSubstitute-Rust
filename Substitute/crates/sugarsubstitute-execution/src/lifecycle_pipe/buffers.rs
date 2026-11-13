// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Return fixed admitted message buffers through passive guards, never native destructors.
use super::*;

/// Original captured metadata allowance, including the sole mapped startup section.
pub(crate) const CAPTURED_METADATA_BYTES: usize = 131_072;
/// Captured lifecycle allocation: original metadata plus two fixed message buffers.
pub const MIN_CAPTURED_LIFECYCLE_PIPE_BYTES: u64 =
    CAPTURED_METADATA_BYTES as u64 + 2 * MAX_LIFECYCLE_MESSAGE_BYTES as u64;
use sugarsubstitute_contracts::execution::{MemoryClaim, MemoryLease};
pub(crate) struct Buffer {
    pub(crate) bytes: Box<[u8]>,
    _claim: MemoryClaim,
}
impl Buffer {
    pub(crate) fn new(memory: &MemoryLease) -> Result<Self, LifecyclePipeFailure> {
        let claim = memory.claim(MAX_LIFECYCLE_MESSAGE_BYTES as u64).map_err(|e| {
            LifecyclePipeFailure::native(NativeObservationError::new("pipe_buffer_claim", e))
        })?;
        Ok(Self { bytes: vec![0; MAX_LIFECYCLE_MESSAGE_BYTES].into_boxed_slice(), _claim: claim })
    }
}
/// Own the single preclaimed outgoing buffer while encoding a complete wire message.
/// Drop returns storage and wakes its owner without closing any native handle. Holding it never
/// delays the session's retirement; after retirement, drop frees the storage and its charge.
pub struct LifecyclePipeWriteBuffer {
    pub(crate) buffer: Option<Buffer>,
    pub(crate) shared: SharedRef,
}
impl LifecyclePipeWriteBuffer {
    /// Borrow writable fixed storage for the protocol owner's preflighted encoder.
    pub fn as_mut_bytes(&mut self) -> &mut [u8] {
        self.buffer.as_mut().map_or(&mut [], |b| b.bytes.as_mut())
    }
}
impl Drop for LifecyclePipeWriteBuffer {
    fn drop(&mut self) {
        if let Some(buffer) = self.buffer.take() {
            let retired = {
                let mut state = lock(&self.shared.state);
                state.loans -= 1;
                if state.retired {
                    Some(buffer)
                } else {
                    state.write_buffer = Some(buffer);
                    None
                }
            };
            // Freed outside the session lock; storage drops before its memory claim.
            drop(retired);
            self.shared.changed();
        }
    }
}
/// Own one immutable complete native message and its original charged allocation.
/// Holding this guard applies bounded read backpressure; bytes are secret-redacted. It never
/// delays the session's retirement; after retirement, drop frees the storage and its charge.
pub struct ReceivedLifecycleFrame {
    pub(crate) buffer: Option<Buffer>,
    pub(crate) length: usize,
    pub(crate) shared: SharedRef,
}
impl ReceivedLifecycleFrame {
    /// Borrow exact observed bytes; decoding grants no peer or resource authority.
    pub fn as_bytes(&self) -> &[u8] {
        self.buffer.as_ref().map_or(&[], |buffer| &buffer.bytes[..self.length])
    }
}
impl Drop for ReceivedLifecycleFrame {
    fn drop(&mut self) {
        if let Some(buffer) = self.buffer.take() {
            let retired = {
                let mut s = lock(&self.shared.state);
                s.loans -= 1;
                if s.retired {
                    Some(buffer)
                } else {
                    s.read_buffer = Some(buffer);
                    None
                }
            };
            drop(retired);
            self.shared.changed();
        }
    }
}
