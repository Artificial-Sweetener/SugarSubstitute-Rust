// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Check an archive's header and digest incrementally, without decompressing its body.
use super::{
    ArchiveError, HEADER_BYTES, INPUT_ARCHIVE_VERSION, MAGIC, MAX_ARCHIVE_BYTES, MAX_PAYLOAD_BYTES,
};
use sha2::{Digest, Sha256};

/// Streaming integrity check of archive bytes: magic, version, flags, declared bounds, exact
/// length and digest. Storage adapters use it to classify a file without buffering it; decoding
/// applies the same check before decompressing.
#[derive(Clone, Debug)]
pub struct ArchiveIntegrity {
    header: [u8; HEADER_BYTES],
    received: usize,
    body: u64,
    body_received: u64,
    hasher: Sha256,
}
impl Default for ArchiveIntegrity {
    fn default() -> Self {
        Self::new()
    }
}
impl ArchiveIntegrity {
    /// Begin checking a new byte stream.
    pub fn new() -> Self {
        Self {
            header: [0; HEADER_BYTES],
            received: 0,
            body: 0,
            body_received: 0,
            hasher: Sha256::new(),
        }
    }

    /// Check the complete bytes of one archive.
    /// # Errors
    /// As [`Self::update`] and [`Self::finish`].
    pub fn check(bytes: &[u8]) -> Result<u64, ArchiveError> {
        let mut integrity = Self::new();
        integrity.update(bytes)?;
        integrity.finish()
    }

    /// Consume the next bytes of the stream.
    /// # Errors
    /// Rejects foreign magic, unknown flags and bytes past the declared body as corrupt, an
    /// unknown version as unsupported, and declared lengths beyond their bounds as oversized,
    /// as soon as the offending bytes arrive.
    pub fn update(&mut self, mut bytes: &[u8]) -> Result<(), ArchiveError> {
        if self.received < HEADER_BYTES {
            let count = bytes.len().min(HEADER_BYTES - self.received);
            self.header[self.received..self.received + count].copy_from_slice(&bytes[..count]);
            self.received += count;
            bytes = &bytes[count..];
            self.check_header()?;
        }
        if bytes.is_empty() {
            return Ok(());
        }
        let count = bytes.len() as u64;
        if count > self.body - self.body_received {
            return Err(ArchiveError::Corrupt("Bytes follow the archive body"));
        }
        self.body_received += count;
        self.hasher.update(bytes);
        Ok(())
    }

    /// Complete the check and return the declared uncompressed payload length.
    /// # Errors
    /// Rejects a stream that ended early as truncated and a digest mismatch as corrupt.
    pub fn finish(self) -> Result<u64, ArchiveError> {
        if self.received < HEADER_BYTES || self.body_received < self.body {
            return Err(ArchiveError::Truncated);
        }
        let payload = self.payload();
        if self.hasher.finalize()[..] != self.header[32..] {
            return Err(ArchiveError::Corrupt("Archive digest does not match its content"));
        }
        Ok(payload)
    }

    fn check_header(&mut self) -> Result<(), ArchiveError> {
        let header = &self.header[..self.received];
        if header.len() >= 8 && header[..8] != MAGIC {
            return Err(ArchiveError::Corrupt("Not an input-document archive"));
        }
        if header.len() >= 12 {
            let version = u32::from_le_bytes(field(header, 8));
            if version != INPUT_ARCHIVE_VERSION {
                return Err(ArchiveError::UnsupportedVersion(version));
            }
        }
        if header.len() < HEADER_BYTES {
            return Ok(());
        }
        if u32::from_le_bytes(field(header, 12)) != 0 {
            return Err(ArchiveError::Corrupt("Unknown archive flags"));
        }
        let body = u64::from_le_bytes(field(header, 24));
        if self.payload() > MAX_PAYLOAD_BYTES || body > MAX_ARCHIVE_BYTES - HEADER_BYTES as u64 {
            return Err(ArchiveError::Oversized);
        }
        self.body = body;
        self.hasher.update(&self.header[..32]);
        Ok(())
    }

    fn payload(&self) -> u64 {
        u64::from_le_bytes(field(&self.header, 16))
    }
}

pub(super) fn field<const N: usize>(bytes: &[u8], offset: usize) -> [u8; N] {
    let mut value = [0; N];
    value.copy_from_slice(&bytes[offset..offset + N]);
    value
}
