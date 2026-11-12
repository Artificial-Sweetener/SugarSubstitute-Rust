// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Deterministically signed local repository fixtures; no production keys or network access.
use crate::MetadataSet;
use chrono::{DateTime, Utc};
use ring::signature::KeyPair;
use tuf::{
    crypto::{Ed25519PrivateKey, HashAlgorithm, PrivateKey},
    metadata::{
        RootMetadataBuilder, SnapshotMetadataBuilder, TargetPath, TargetsMetadataBuilder,
        TimestampMetadataBuilder,
    },
    pouf::Pouf1,
};

/// Fixed verification time for deterministic boundary tests.
pub const NOW: i64 = 1_700_000_000;
/// Fixed metadata expiry for deterministic boundary tests.
pub const EXPIRES: i64 = NOW + 3600;
/// Arbitrary target used by signature and native-storage tests.
pub const NAME: &str = "channels/release/windows-x86_64.json";

/// Derive a public test key; never use fixture seeds in a distributed trust root.
pub fn key(seed: u8) -> Result<Ed25519PrivateKey, Box<dyn std::error::Error + Send + Sync>> {
    let key = ring::signature::Ed25519KeyPair::from_seed_unchecked(&[seed; 32])
        .map_err(|_| "Invalid fixture key")?;
    let bytes = [&[seed; 32][..], key.public_key().as_ref()].concat();
    Ok(Ed25519PrivateKey::from_ed25519(&bytes)?)
}
/// Sign a test root, optionally authorized by the preceding root key.
pub fn root(
    key: &Ed25519PrivateKey,
    version: u32,
    old: Option<&Ed25519PrivateKey>,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    let mut root = RootMetadataBuilder::new()
        .version(version)
        .expires(time(EXPIRES)?)
        .consistent_snapshot(true)
        .root_key(key.public().clone())
        .timestamp_key(key.public().clone())
        .snapshot_key(key.public().clone())
        .targets_key(key.public().clone())
        .signed::<Pouf1>(key)?;
    if let Some(old) = old {
        root.add_signature(old)?;
    }
    Ok(root.to_raw()?.as_bytes().to_vec())
}
/// Exact signed role bytes for hostile storage and transport qualification.
pub struct Set {
    /// Signed timestamp bytes.
    pub timestamp: Vec<u8>,
    /// Signed snapshot bytes.
    pub snapshot: Vec<u8>,
    /// Signed targets bytes.
    pub targets: Vec<u8>,
}
impl Set {
    /// Build the common single-target fixture with explicit role versions and expiry.
    pub fn new(
        key: &Ed25519PrivateKey,
        versions: [u32; 3],
        bytes: &[u8],
        expires: i64,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::targets(key, versions, &[(NAME, bytes)], expires)
    }
    /// Build a signed repository containing the supplied exact target bytes.
    pub fn targets(
        key: &Ed25519PrivateKey,
        versions: [u32; 3],
        entries: &[(&str, &[u8])],
        expires: i64,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let mut targets =
            TargetsMetadataBuilder::new().version(versions[2]).expires(time(expires)?);
        for (name, bytes) in entries {
            targets = targets.insert_target_from_slice(
                TargetPath::new(*name)?,
                bytes,
                &[HashAlgorithm::Sha256],
            )?;
        }
        let targets = targets.signed::<Pouf1>(key)?;
        let snapshot = SnapshotMetadataBuilder::from_targets(&targets, &[HashAlgorithm::Sha256])?
            .version(versions[1])
            .expires(time(expires)?)
            .signed::<Pouf1>(key)?;
        let timestamp =
            TimestampMetadataBuilder::from_snapshot(&snapshot, &[HashAlgorithm::Sha256])?
                .version(versions[0])
                .expires(time(expires)?)
                .signed::<Pouf1>(key)?;
        Ok(Self {
            timestamp: timestamp.to_raw()?.as_bytes().to_vec(),
            snapshot: snapshot.to_raw()?.as_bytes().to_vec(),
            targets: targets.to_raw()?.as_bytes().to_vec(),
        })
    }
    /// Borrow the complete role set for the production verifier.
    pub fn metadata(&self) -> MetadataSet<'_> {
        MetadataSet { timestamp: &self.timestamp, snapshot: &self.snapshot, targets: &self.targets }
    }
}
/// Convert an explicit fixture time without consulting the machine clock.
pub fn time(seconds: i64) -> Result<DateTime<Utc>, Box<dyn std::error::Error + Send + Sync>> {
    DateTime::from_timestamp(seconds, 0).ok_or_else(|| "Invalid fixture timestamp".into())
}
