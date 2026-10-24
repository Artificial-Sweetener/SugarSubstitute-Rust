// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Fixed-memory SHA-256 for streamed package bytes, using the pinned accelerated crypto backend.

/// Incremental SHA-256 shared by download, extraction and installed-payload verification.
/// Hashing grants no authenticity: callers must compare with an authenticated expected digest
/// and retain custody of the exact bytes. Callers own scheduling, chunk bounds and cancellation.
pub struct PayloadHasher(ring::digest::Context);

impl PayloadHasher {
    /// Start a fresh stream with no filesystem, clock or scheduling effects.
    pub fn new() -> Self {
        Self(ring::digest::Context::new(&ring::digest::SHA256))
    }

    /// Include the next exact chunk; memory use does not grow with stream length.
    pub fn update(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }

    /// Consume the stream and return the complete 32-byte SHA-256 digest.
    pub fn finish(self) -> [u8; 32] {
        let mut bytes = [0; 32];
        bytes.copy_from_slice(self.0.finish().as_ref());
        bytes
    }

    /// Consume the stream and return the package inventory's canonical lowercase hex digest.
    pub fn finish_hex(self) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        self.finish()
            .into_iter()
            .flat_map(|byte| {
                [char::from(HEX[(byte >> 4) as usize]), char::from(HEX[(byte & 15) as usize])]
            })
            .collect()
    }
}

impl Default for PayloadHasher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    #[test]
    fn known_sha256_vectors_preserve_the_existing_inventory_encoding() {
        assert_eq!(
            PayloadHasher::new().finish_hex(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        let mut digest = PayloadHasher::new();
        digest.update(b"a");
        digest.update(b"");
        digest.update(b"bc");
        assert_eq!(
            digest.finish_hex(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn arbitrary_chunks_and_padding_boundaries_match_independent_sha256() {
        let source: Vec<u8> = (0..131_073).map(|index| (index % 251) as u8).collect();
        for length in [0, 1, 55, 56, 63, 64, 65, 127, 128, 129, 65535, 65536, 65537, 131073] {
            let expected: [u8; 32] = Sha256::digest(&source[..length]).into();
            for chunk in [1, 7, 64, 257, 65536] {
                let mut digest = PayloadHasher::new();
                for block in source[..length].chunks(chunk) {
                    digest.update(block);
                }
                assert_eq!(digest.finish(), expected, "length={length}, chunk={chunk}");
            }
        }
    }
}
