// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Authenticate an installer's compiled-in release without a clock, for an explicit offline install.
//!
//! An installer embeds one signed metadata set and the exact package it names. Once the shortest
//! role lifetime passes, ordinary verification refuses that set. Offline, and only after the
//! user explicitly chooses it, the installer may still install that exact package: every role
//! must carry valid signatures under the trusted root, the descriptor and package must match the
//! signed targets, and the package bytes must match the compiled-in digest. Freshness is the only
//! guarantee given up, which is why the updater must bring the installation current afterwards.
use crate::{Error, RepositoryTrust, VerifiedOffer, checkpoint::Checkpoint};
use sugarsubstitute_contracts::update::Request;
use tuf::{
    metadata::{
        Metadata, RawSignedMetadata, RootMetadata, SnapshotMetadata, TargetsMetadata,
        TimestampMetadata,
    },
    pouf::Pouf1,
};

impl RepositoryTrust {
    /// Verify `metadata` and `descriptor` under this trust's root as of the last instant at which
    /// every role, the root included, was still within its signed lifetime. Accepted floors and
    /// the clock floor are neither consulted nor changed, so this never advances durable trust.
    /// The returned offer's package target has no validity window: its bytes are still verified
    /// in full against the signed digest and length.
    /// # Errors
    /// Rejects malformed or unsigned roles, signatures by keys the root does not authorise,
    /// inconsistent role bindings, routing mismatches and descriptor or package disagreement.
    pub fn pinned_offer(
        &self,
        metadata: crate::MetadataSet<'_>,
        request: &Request,
        descriptor: &[u8],
    ) -> Result<VerifiedOffer, Error> {
        let root = self.accepted().root.clone();
        let as_of = signed_lifetime_end(metadata, &root)?
            .checked_sub(1)
            .filter(|seconds| *seconds >= 0)
            .ok_or(Error::Bound)?;
        let mut isolated = Self::from_trusted_checkpoint(&Checkpoint::new(root).encode()?)?;
        isolated.refresh(metadata, as_of)?;
        Ok(isolated.offer(request, descriptor, as_of)?.pinned())
    }
}

/// Unix UTC second at which the earliest of these roles and `root` expires, read without
/// verifying signatures: use it to decide whether a set is stale, never to trust it.
/// # Errors
/// Rejects empty, oversized or unparsable roles.
pub fn signed_lifetime_end(metadata: crate::MetadataSet<'_>, root: &[u8]) -> Result<i64, Error> {
    [
        expiry::<RootMetadata>(root)?,
        expiry::<TimestampMetadata>(metadata.timestamp)?,
        expiry::<SnapshotMetadata>(metadata.snapshot)?,
        expiry::<TargetsMetadata>(metadata.targets)?,
    ]
    .into_iter()
    .min()
    .ok_or(Error::Incomplete)
}

/// Snapshot version an unverified timestamp names, used only to choose which file to fetch.
/// # Errors
/// Rejects empty, oversized or unparsable bytes.
pub fn named_snapshot(timestamp: &[u8]) -> Result<u32, Error> {
    Ok(untrusted::<TimestampMetadata>(timestamp)?.snapshot().version())
}

/// Targets version an unverified snapshot names, used only to choose which file to fetch.
/// # Errors
/// Rejects empty, oversized or unparsable bytes, or a snapshot that names no targets.
pub fn named_targets(snapshot: &[u8]) -> Result<u32, Error> {
    Ok(untrusted::<SnapshotMetadata>(snapshot)?
        .meta()
        .get(&tuf::metadata::MetadataPath::targets())
        .ok_or(Error::Incomplete)?
        .version())
}

/// Signed expiry of one role, read before its signatures are checked only to choose `as_of`;
/// the subsequent verification rejects the role if it was not genuinely signed.
fn expiry<M: Metadata>(bytes: &[u8]) -> Result<i64, Error> {
    Ok(untrusted::<M>(bytes)?.expires().timestamp())
}

/// Parse a bounded role without checking its signatures.
fn untrusted<M: Metadata>(bytes: &[u8]) -> Result<M, Error> {
    if bytes.is_empty() || bytes.len() > crate::MAX_METADATA_BYTES {
        return Err(Error::Bound);
    }
    let metadata = RawSignedMetadata::<Pouf1, M>::new(bytes.to_vec()).parse_untrusted()?;
    Ok(metadata.assume_valid()?)
}

#[cfg(test)]
mod tests;
