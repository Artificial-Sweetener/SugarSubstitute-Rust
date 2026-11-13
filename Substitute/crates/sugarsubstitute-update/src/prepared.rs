// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decide whether a prepared update may start on its local receipt once signed metadata is stale.
//!
//! A prepared update is verified under current signed authority before its receipt is written.
//! Metadata lifetimes are shorter than the time between some users' sessions, so a later launch
//! may find the cached metadata expired or missing. The receipt then stands in for that metadata
//! while every condition in [`RepositoryTrust::admit_prepared`] still holds. Revocation is
//! honoured only through metadata this installation has actually authenticated; without a
//! connection nothing newer can be learned, and an unverifiable newer listing refuses.
use crate::{
    Error, ReleaseHistory, RepositoryTrust, VerifiedOffer, checkpoint::Checkpoint,
    releases::RouteFloor,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sugarsubstitute_contracts::update::{Channel, Component, Platform, Request};
use tuf::{
    crypto::HashAlgorithm,
    metadata::{RawSignedMetadata, TargetDescription, TargetPath, TargetsMetadata},
    pouf::Pouf1,
};

/// Longest time, in seconds, that a receipt alone may stand in for expired signed metadata.
pub const RECEIPT_LIFETIME_SECONDS: i64 = 90 * 24 * 60 * 60;

/// Release facts bound under the trust writer at the moment a prepared update was committed.
/// Additive fields may be appended; readers ignore fields they do not know.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PreparedRelease {
    /// Subscription channel the release was authorised for.
    pub channel: Channel,
    /// Native component; only the application activates through a receipt.
    pub component: Component,
    /// Native platform of the prepared package.
    pub platform: Platform,
    /// Signed release sequence used for rollback protection.
    pub sequence: u64,
    /// Signed package target name.
    pub package_name: String,
    /// Signed package length in bytes.
    pub package_bytes: u64,
    /// Signed SHA-256 digest of the package archive.
    pub package_digest: [u8; 32],
    /// SHA-256 of the exact signed channel descriptor bytes that named this package.
    pub descriptor_digest: [u8; 32],
    /// Version of the trusted root in force when the receipt was written.
    pub root_version: u32,
    /// Version of the authenticated targets metadata that listed this release.
    pub targets_version: u32,
    /// Unix UTC seconds from the preparation's injected clock when the receipt was written.
    pub received_at: i64,
}

/// Why a receipt cannot start its prepared update; the accepted version starts instead.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReceiptRefusal {
    /// The receipt describes another channel, component or platform than this request.
    Route,
    /// The trusted root was rotated (or replaced) after the receipt was written.
    RootAdvanced,
    /// The release sequence is below this installation's release floor.
    BelowFloor,
    /// The receipt is older than [`RECEIPT_LIFETIME_SECONDS`].
    Aged,
    /// The supplied time precedes the receipt or a previously trusted verification.
    ClockRollback,
    /// Newer authenticated metadata no longer lists the descriptor or package.
    Revoked,
    /// Newer authenticated metadata lists a different descriptor for this subscription.
    Superseded,
    /// Newer metadata was authenticated, but its exact bytes are not available to inspect.
    NewerMetadataUnavailable,
    /// Local trust is older than the receipt it is asked to judge.
    Inconsistent,
}
impl std::fmt::Display for ReceiptRefusal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Route => "Prepared update receipt belongs to another subscription",
            Self::RootAdvanced => "Update signing authority changed after preparation",
            Self::BelowFloor => "Prepared update is older than an already authorised release",
            Self::Aged => "Prepared update receipt is too old to start offline",
            Self::ClockRollback => "System time is earlier than the prepared update receipt",
            Self::Revoked => "Newer signed metadata withdrew the prepared update",
            Self::Superseded => "Newer signed metadata replaced the prepared update",
            Self::NewerMetadataUnavailable => "Newer signed metadata could not be inspected",
            Self::Inconsistent => "Update trust is older than the prepared update receipt",
        })
    }
}
impl std::error::Error for ReceiptRefusal {}

impl RepositoryTrust {
    /// Receipt facts for `offer`, which this trust authenticated against `descriptor` at `now`.
    /// Call inside the same exclusive hold that verified the offer and publishes the receipt.
    /// # Errors
    /// Rejects an incomplete refresh, a descriptor that is not this offer's, or another route.
    pub fn prepared_release(
        &self,
        request: &Request,
        offer: &VerifiedOffer,
        descriptor: &[u8],
        now: i64,
    ) -> Result<PreparedRelease, Error> {
        let verified = self.target(&request.target_name(), now)?;
        let descriptor_digest: [u8; 32] = Sha256::digest(descriptor).into();
        if verified.digest() != descriptor_digest
            || verified.length() != descriptor.len() as u64
            || offer.offer().admission(request).is_err()
        {
            return Err(Error::Digest);
        }
        let signed = offer.offer();
        Ok(PreparedRelease {
            channel: request.channel,
            component: request.component,
            platform: request.platform,
            sequence: signed.sequence(),
            package_name: signed.package_name(),
            package_bytes: signed.package_bytes(),
            package_digest: signed.package_digest(),
            descriptor_digest,
            root_version: self.root_version()?,
            targets_version: self.accepted().targets_floor().0,
            received_at: now,
        })
    }

    /// Admit a prepared update on its receipt alone (D22). All of these must hold:
    /// - the receipt names this request's route;
    /// - the trusted root version equals the one recorded (a rotation requires a refresh);
    /// - the release sequence is not below durable `history` or the request's healthy floor;
    /// - `now` is not before the receipt or the last trusted verification, and the receipt is
    ///   at most [`RECEIPT_LIFETIME_SECONDS`] old;
    /// - no newer targets metadata authenticated since then omits or supersedes the release.
    ///
    /// `newest_targets` are retained signed targets bytes, if any; they are used only when their
    /// digest equals the newest targets this trust accepted, so they need no fresh signature
    /// time. `now` is Unix UTC seconds from the caller's injected clock. Checks run in this order
    /// so the most fundamental refusal is reported.
    /// # Errors
    /// Returns the first failing condition; nothing is admitted on any refusal.
    pub fn admit_prepared(
        &self,
        release: &PreparedRelease,
        request: &Request,
        history: &ReleaseHistory,
        newest_targets: Option<&[u8]>,
        now: i64,
    ) -> Result<(), ReceiptRefusal> {
        if release.channel != request.channel
            || release.component != request.component
            || release.platform != request.platform
        {
            return Err(ReceiptRefusal::Route);
        }
        if self.root_version().map_err(|_| ReceiptRefusal::Inconsistent)? != release.root_version {
            return Err(ReceiptRefusal::RootAdvanced);
        }
        if release.sequence < request.sequence_floor
            || history.floor(request).is_some_and(|floor: RouteFloor| {
                release.sequence < floor.sequence
                    || (release.sequence == floor.sequence
                        && release.package_digest != floor.digest)
            })
        {
            return Err(ReceiptRefusal::BelowFloor);
        }
        if now < release.received_at || now < self.accepted().time_floor {
            return Err(ReceiptRefusal::ClockRollback);
        }
        if now - release.received_at > RECEIPT_LIFETIME_SECONDS {
            return Err(ReceiptRefusal::Aged);
        }
        newer_listing(self.accepted(), release, request, newest_targets)
    }
}

/// Inspect the newest authenticated targets only when they are newer than the receipt's.
fn newer_listing(
    checkpoint: &Checkpoint,
    release: &PreparedRelease,
    request: &Request,
    newest_targets: Option<&[u8]>,
) -> Result<(), ReceiptRefusal> {
    let (version, digest) = checkpoint.targets_floor();
    if version < release.targets_version {
        return Err(ReceiptRefusal::Inconsistent);
    }
    // A snapshot may reference newer targets that were never fetched: unseen, so unverifiable.
    if checkpoint.minimum_targets > version {
        return Err(ReceiptRefusal::NewerMetadataUnavailable);
    }
    if version == release.targets_version {
        return Ok(());
    }
    let bytes = newest_targets
        .filter(|bytes| <[u8; 32]>::from(Sha256::digest(bytes)) == digest)
        .ok_or(ReceiptRefusal::NewerMetadataUnavailable)?;
    // These exact bytes were authenticated when the protected floor advanced to their digest.
    let targets = RawSignedMetadata::<Pouf1, TargetsMetadata>::new(bytes.to_vec())
        .parse_untrusted()
        .and_then(|signed| signed.assume_valid())
        .map_err(|_| ReceiptRefusal::NewerMetadataUnavailable)?;
    if tuf::metadata::Metadata::version(&targets) != version {
        return Err(ReceiptRefusal::NewerMetadataUnavailable);
    }
    let listed = |name: &str| {
        TargetPath::new(name).ok().and_then(|path| targets.targets().get(&path).cloned())
    };
    let descriptor = listed(&request.target_name()).ok_or(ReceiptRefusal::Revoked)?;
    if !describes(&descriptor, release.descriptor_digest, None) {
        return Err(ReceiptRefusal::Superseded);
    }
    let package = listed(&release.package_name).ok_or(ReceiptRefusal::Revoked)?;
    if !describes(&package, release.package_digest, Some(release.package_bytes)) {
        return Err(ReceiptRefusal::Revoked);
    }
    Ok(())
}

/// Whether a signed description names exactly these SHA-256 bytes (and length, when known).
fn describes(description: &TargetDescription, digest: [u8; 32], length: Option<u64>) -> bool {
    description.hashes().get(&HashAlgorithm::Sha256).is_some_and(|hash| hash.value() == digest)
        && length.is_none_or(|length| description.length() == length)
}

#[cfg(test)]
mod tests;
