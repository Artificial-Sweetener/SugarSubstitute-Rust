// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own one preclaimed image allocation through native writes and immutable transfer.

use crate::DesktopError;
use std::{error::Error, fmt};
use sugarsubstitute_contracts::execution::{MemoryClaim, MemoryLease};

/// Maximum encoded native preview allocation; compression overflow remains observable failure.
pub(super) const IMAGE_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum BufferFailure {
    Full,
    InvalidPosition,
    Frozen,
    MissingStorage,
}

impl fmt::Display for BufferFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Native preview storage rejected: {self:?}")
    }
}
impl Error for BufferFailure {}

/// An extracted image retains its original main reservation until final disposal.
pub(super) struct Image {
    pub(super) bytes: Vec<u8>,
    pub(super) claim: MemoryClaim,
}

/// Native writes cannot grow the allocation, and the first failure invalidates all bytes.
pub(super) struct Buffer {
    bytes: Vec<u8>,
    position: usize,
    length: usize,
    frozen: bool,
    failure: Option<BufferFailure>,
    claim: Option<MemoryClaim>,
}

impl Buffer {
    /// Claim before allocation and reject allocator capacity exceeding admitted bytes.
    pub(super) fn new(lease: &MemoryLease) -> Result<Self, DesktopError> {
        Self::allocate(lease, IMAGE_BYTES)
    }

    fn allocate(lease: &MemoryLease, capacity: usize) -> Result<Self, DesktopError> {
        let claim = lease
            .claim(capacity as u64)
            .map_err(|cause| DesktopError::new("claim_main_preview", cause))?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(capacity)
            .map_err(|cause| DesktopError::new("allocate_main_preview", cause))?;
        if bytes.capacity() != capacity {
            return Err(DesktopError::message(
                "allocate_main_preview",
                "preview allocation exceeds admitted capacity",
            ));
        }
        bytes.resize(capacity, 0);
        Ok(Self { bytes, position: 0, length: 0, frozen: false, failure: None, claim: Some(claim) })
    }

    /// Preserve the first stream error even when native capture later reports success.
    pub(super) fn fail<T>(&mut self, cause: BufferFailure) -> Result<T, BufferFailure> {
        let retained = *self.failure.get_or_insert(cause);
        Err(retained)
    }

    /// Copy an admitted native write atomically; rejected writes modify no bytes or cursor.
    pub(super) fn write(&mut self, input: &[u8]) -> Result<(), BufferFailure> {
        self.writable()?;
        let Some(end) = self.position.checked_add(input.len()) else {
            return self.fail(BufferFailure::Full);
        };
        if end > self.bytes.len() {
            return self.fail(BufferFailure::Full);
        }
        self.bytes[self.position..end].copy_from_slice(input);
        self.position = end;
        self.length = self.length.max(end);
        Ok(())
    }

    /// Read only the actual written extent; an exhausted stream returns zero bytes.
    pub(super) fn read(&mut self, output: &mut [u8]) -> Result<usize, BufferFailure> {
        self.healthy()?;
        if self.frozen {
            return self.fail(BufferFailure::Frozen);
        }
        let count = output.len().min(self.length.saturating_sub(self.position));
        let end = self.position + count;
        output[..count].copy_from_slice(&self.bytes[self.position..end]);
        self.position = end;
        Ok(count)
    }

    /// Resolve checked signed seeks against an explicit origin without changing the extent.
    pub(super) fn seek(&mut self, displacement: i64, origin: u32) -> Result<u64, BufferFailure> {
        self.writable()?;
        let base = match origin {
            0 => 0,
            1 => self.position as i64,
            2 => self.length as i64,
            _ => return self.fail(BufferFailure::InvalidPosition),
        };
        let Some(position) = base.checked_add(displacement) else {
            return self.fail(BufferFailure::InvalidPosition);
        };
        let Ok(position) = usize::try_from(position) else {
            return self.fail(BufferFailure::InvalidPosition);
        };
        if position > self.bytes.len() {
            return self.fail(BufferFailure::InvalidPosition);
        }
        self.position = position;
        Ok(position as u64)
    }

    /// Resize the logical extent within preallocated zeroed storage.
    pub(super) fn set_size(&mut self, size: u64) -> Result<(), BufferFailure> {
        self.writable()?;
        let Ok(size) = usize::try_from(size) else { return self.fail(BufferFailure::Full) };
        if size > self.bytes.len() {
            return self.fail(BufferFailure::Full);
        }
        if size < self.length {
            self.bytes[size..self.length].fill(0);
        }
        self.length = size;
        Ok(())
    }

    /// Observe a scalar extent without allocating native names or copying image bytes.
    pub(super) fn length(&self) -> usize {
        self.length
    }

    /// Freeze only after actual native completion; failed bytes never become an image.
    pub(super) fn freeze(&mut self) -> Result<(), BufferFailure> {
        self.healthy()?;
        if self.frozen {
            return self.fail(BufferFailure::Frozen);
        }
        self.frozen = true;
        Ok(())
    }

    /// Move the original allocation and claim once, after callback return and stream quiescence.
    pub(super) fn take(&mut self) -> Result<Image, BufferFailure> {
        self.healthy()?;
        if !self.frozen {
            return self.fail(BufferFailure::InvalidPosition);
        }
        let Some(claim) = self.claim.take() else {
            return self.fail(BufferFailure::MissingStorage);
        };
        let mut bytes = std::mem::take(&mut self.bytes);
        bytes.truncate(self.length);
        Ok(Image { bytes, claim })
    }

    fn healthy(&self) -> Result<(), BufferFailure> {
        self.failure.map_or(Ok(()), Err)
    }

    fn writable(&mut self) -> Result<(), BufferFailure> {
        self.healthy()?;
        if self.frozen { self.fail(BufferFailure::Frozen) } else { Ok(()) }
    }
}

#[cfg(test)]
#[path = "buffer_tests.rs"]
mod tests;
