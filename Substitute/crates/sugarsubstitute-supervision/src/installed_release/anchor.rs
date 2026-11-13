// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Select the immutable public identity that signs this build's release manifest.
//!
//! A release build embeds the release anchor named by `SUGARSUBSTITUTE_RELEASE_ANCHOR` at
//! build time; the explicit local K1 build embeds the reviewed development anchor instead.
//! Any other build has no trusted release identity.

/// Borrow the literal public data; callers cannot supply a signer.
pub(super) struct Anchor {
    /// SHA-256 of the canonical 411-byte CNG RSA public blob, as signed manifests name it.
    pub(super) key_id: [u8; 32],
    pub(super) public_blob: &'static [u8; 411],
}

#[cfg(not(feature = "local-k1-symbol-trust"))]
static RELEASE_ANCHOR: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/release-anchor.bin"));

/// Return the compiled anchor, or `None` when this build trusts no release identity.
pub(super) fn selected() -> Option<Anchor> {
    #[cfg(feature = "local-k1-symbol-trust")]
    {
        Some(Anchor {
            key_id: super::local_k1_anchor::KEY_ID,
            public_blob: &super::local_k1_anchor::PUBLIC_BLOB,
        })
    }
    #[cfg(not(feature = "local-k1-symbol-trust"))]
    {
        embedded(RELEASE_ANCHOR)
    }
}

/// Admit an embedded blob only in its exact RSA-3072, exponent 65537 CNG shape.
#[cfg(any(test, not(feature = "local-k1-symbol-trust")))]
fn embedded(bytes: &'static [u8]) -> Option<Anchor> {
    use sha2::{Digest, Sha256};
    const HEADER: [u8; 27] = [
        0x52, 0x53, 0x41, 0x31, 0x00, 0x0c, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x80, 0x01, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01,
    ];
    let public_blob: &'static [u8; 411] = bytes.try_into().ok()?;
    if public_blob[..HEADER.len()] != HEADER {
        return None;
    }
    Some(Anchor { key_id: Sha256::digest(public_blob).into(), public_blob })
}

#[cfg(test)]
mod tests;
