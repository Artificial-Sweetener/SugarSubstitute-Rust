// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind protected checkpoint bytes to an installation and stable repository identity.
//! The checksum detects damaged storage; filesystem protection supplies local trust.
use sha2::{Digest, Sha256};
use std::io;
use sugarsubstitute_contracts::installation::InstallationId;
use sugarsubstitute_update::MAX_CHECKPOINT_BYTES;

/// Separate record domains cannot substitute for one another within the same installation.
#[derive(Clone, Copy)]
pub(super) enum Kind {
    Checkpoint,
    Releases,
    Continuity,
    Offer,
    InstalledRelease,
    Subscription,
    SubscriptionMarker,
    UpdaterSelection,
    UpdaterMarker,
    UpdaterRelease,
}
impl Kind {
    fn magic(self) -> &'static [u8; 8] {
        match self {
            Self::Checkpoint => b"SSTRUST1",
            Self::Releases => b"SSRELSE1",
            Self::Continuity => b"SSTRCON1",
            Self::Offer => b"SSOFFER1",
            Self::InstalledRelease => b"SSINSTL1",
            Self::Subscription => b"SSSUBSC1",
            Self::SubscriptionMarker => b"SSSUBCM1",
            Self::UpdaterSelection => b"SSENGSL1",
            Self::UpdaterMarker => b"SSENGSM1",
            Self::UpdaterRelease => b"SSENGRL1",
        }
    }
    fn payload_limit(self) -> usize {
        match self {
            Self::Checkpoint => MAX_CHECKPOINT_BYTES,
            Self::Releases => sugarsubstitute_update::MAX_RELEASE_HISTORY_BYTES,
            Self::Continuity => 64,
            Self::Offer => super::offer_cache::MAX_PAYLOAD_BYTES,
            Self::InstalledRelease => super::receipt::MAX_BYTES,
            Self::Subscription => sugarsubstitute_contracts::update::subscription::MAX_BYTES,
            Self::SubscriptionMarker => 64,
            Self::UpdaterSelection => sugarsubstitute_contracts::installation::MAX_RECORD_BYTES,
            Self::UpdaterMarker => 64,
            Self::UpdaterRelease => super::receipt::MAX_BYTES,
        }
    }
    pub(super) fn limit(self) -> usize {
        HEADER + self.payload_limit() + DIGEST
    }
}
const HEADER: usize = 8 + 32 + 32;
const DIGEST: usize = 32;

/// Encode without expanding the already bounded checkpoint into a JSON byte array.
pub(super) fn encode(
    kind: Kind,
    installation: &InstallationId,
    repository: &[u8; 32],
    checkpoint: &[u8],
) -> io::Result<Vec<u8>> {
    if checkpoint.is_empty() || checkpoint.len() > kind.payload_limit() {
        return Err(invalid());
    }
    let mut bytes = Vec::with_capacity(HEADER + checkpoint.len() + DIGEST);
    bytes.extend_from_slice(kind.magic());
    bytes.extend_from_slice(installation.as_str().as_bytes());
    bytes.extend_from_slice(repository);
    bytes.extend_from_slice(checkpoint);
    let digest = Sha256::digest(&bytes);
    bytes.extend_from_slice(&digest);
    Ok(bytes)
}

/// Reject foreign state, unknown versions, corruption and oversized input without adopting it.
pub(super) fn decode<'a>(
    kind: Kind,
    installation: &InstallationId,
    repository: &[u8; 32],
    bytes: &'a [u8],
) -> io::Result<&'a [u8]> {
    if bytes.len() <= HEADER + DIGEST || bytes.len() > kind.limit() {
        return Err(invalid());
    }
    let end = bytes.len() - DIGEST;
    if &bytes[..8] != kind.magic()
        || &bytes[8..40] != installation.as_str().as_bytes()
        || &bytes[40..HEADER] != repository
        || Sha256::digest(&bytes[..end])[..] != bytes[end..]
    {
        return Err(invalid());
    }
    Ok(&bytes[HEADER..end])
}

/// Preserve untrusted bytes and require explicit repair rather than resetting security state.
fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "Update trust record is damaged or belongs elsewhere",
    )
}
