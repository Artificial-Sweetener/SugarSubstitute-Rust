// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Incremental payload verification with fixed memory and no transport or scheduling effects.
use crate::package::PayloadHasher;
use crate::{Error, MAX_TARGET_BYTES};
use tuf::{crypto::HashAlgorithm, metadata::TargetDescription};

/// Authenticated identity and limits for one exact repository target.
pub struct Target {
    name: String,
    length: u64,
    digest: [u8; 32],
    not_before: i64,
    not_after: i64,
}
impl Target {
    /// Bind a verified TUF target description to its complete metadata validity interval.
    pub(super) fn new(
        name: &str,
        description: &TargetDescription,
        not_before: i64,
        not_after: i64,
    ) -> Result<Self, Error> {
        let digest = description
            .hashes()
            .get(&HashAlgorithm::Sha256)
            .ok_or(Error::Digest)?
            .value()
            .try_into()
            .map_err(|_| Error::Digest)?;
        if description.length() > MAX_TARGET_BYTES {
            return Err(Error::Bound);
        }
        Ok(Self { name: name.into(), length: description.length(), digest, not_before, not_after })
    }
    /// Signed relative repository name; the transport must still enforce its origin policy.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Signed expected payload length in bytes, independent of Content-Length.
    pub fn length(&self) -> u64 {
        self.length
    }
    /// Signed SHA-256 digest used for immutable staging identity.
    pub fn digest(&self) -> [u8; 32] {
        self.digest
    }
    /// Begin a fresh pass over all target bytes, including any resumed prefix.
    /// # Errors
    /// Rejects use outside this metadata's verified time interval.
    pub fn verifier(self, now: i64) -> Result<TargetVerifier, Error> {
        self.require_valid_at(now)?;
        Ok(TargetVerifier {
            target: self,
            digest: PayloadHasher::new(),
            received: 0,
            failed: false,
        })
    }
    /// Check the signed metadata interval before admitting transport or cache writes.
    /// `now` is Unix UTC seconds. This does not verify bytes or authorize installation.
    /// # Errors
    /// Rejects clock rollback or expired metadata; final byte verification rechecks this interval.
    pub fn require_valid_at(&self, now: i64) -> Result<(), Error> {
        if now < self.not_before {
            Err(Error::ClockRollback)
        } else if now >= self.not_after {
            Err(Error::Expired)
        } else {
            Ok(())
        }
    }
}

/// Fixed-memory incremental verifier; an error permanently invalidates the current stream.
pub struct TargetVerifier {
    target: Target,
    digest: PayloadHasher,
    received: u64,
    failed: bool,
}
impl TargetVerifier {
    /// Include the next exact received chunk, without trusting transport size declarations.
    /// # Errors
    /// Rejects overlong data, counter overflow or continuation after any prior failure.
    pub fn push(&mut self, bytes: &[u8]) -> Result<(), Error> {
        if self.failed {
            return Err(Error::Digest);
        }
        let total = self
            .received
            .checked_add(bytes.len() as u64)
            .filter(|total| *total <= self.target.length);
        let Some(total) = total else {
            self.failed = true;
            return Err(Error::Bound);
        };
        self.digest.update(bytes);
        self.received = total;
        Ok(())
    }
    /// Authenticate the complete stream at the supplied Unix UTC time.
    /// # Errors
    /// Rejects truncated, altered or previously overlong data and metadata expired mid-download.
    pub fn finish(self, now: i64) -> Result<VerifiedBytes, Error> {
        self.target.require_valid_at(now)?;
        if self.failed
            || self.received != self.target.length
            || self.digest.finish() != self.target.digest
        {
            return Err(Error::Digest);
        }
        Ok(VerifiedBytes { target: self.target })
    }
}
/// Evidence about bytes actually streamed; native storage must retain their file custody.
/// This receipt alone never authorizes opening a different or subsequently modified file.
/// Revalidate current metadata and subscription before activation; key rotation or a newer
/// channel descriptor can retire an earlier download's authorization while preserving its bytes.
pub struct VerifiedBytes {
    target: Target,
}
impl VerifiedBytes {
    /// Exact authenticated target name.
    pub fn name(&self) -> &str {
        &self.target.name
    }
    /// Verified byte length.
    pub fn length(&self) -> u64 {
        self.target.length
    }
    /// Verified SHA-256 digest.
    pub fn digest(&self) -> [u8; 32] {
        self.target.digest
    }
}
