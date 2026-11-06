// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind a confirmed encoded digest and length without granting backend or filesystem authority.

use std::{error::Error, fmt};

/// Exact original encoded bytes expected by a durable output receipt.
/// This value grants no path, backend, process or cache authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OutputArtifactIdentity {
    digest: [u8; 32],
    bytes: u64,
}

impl OutputArtifactIdentity {
    /// Adopt a canonical lowercase SHA-256 hex digest and its exact positive encoded length.
    /// The source transfer's narrower format and byte limits remain authoritative.
    /// # Errors
    /// Rejects noncanonical digests, empty artifacts and lengths above the 8 GiB session quota.
    pub fn new(digest: &str, bytes: u64) -> Result<Self, OutputArtifactIdentityError> {
        if digest.len() != 64 || bytes == 0 || bytes > 8 * 1024 * 1024 * 1024 {
            return Err(OutputArtifactIdentityError);
        }
        let mut decoded = [0u8; 32];
        for (index, pair) in digest.as_bytes().chunks_exact(2).enumerate() {
            let digit = |byte| match byte {
                b'0'..=b'9' => Some(byte - b'0'),
                b'a'..=b'f' => Some(byte - b'a' + 10),
                _ => None,
            };
            decoded[index] = digit(pair[0]).ok_or(OutputArtifactIdentityError)? * 16
                + digit(pair[1]).ok_or(OutputArtifactIdentityError)?;
        }
        Ok(Self { digest: decoded, bytes })
    }

    /// Borrow the expected SHA-256 bytes in digest order.
    pub fn digest(&self) -> &[u8; 32] {
        &self.digest
    }

    /// Return the exact original encoded byte count, independent of decoded image memory.
    pub fn bytes(&self) -> u64 {
        self.bytes
    }
}

/// Explain malformed portable artifact identity without exposing selected file paths.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OutputArtifactIdentityError;

impl fmt::Display for OutputArtifactIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Output artifact digest or encoded length is invalid")
    }
}
impl Error for OutputArtifactIdentityError {}
