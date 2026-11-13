// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Authenticate the published repository before any new metadata is signed on top of it.
//!
//! Signatures, the root chain and every role's binding to the role that references it are
//! checked, but not expiry: re-signing must still work after a missed refresh let a role expire.
//! Expiry is the application's concern, checked by [crate::verify] on the result.
use crate::{Error, invalid};
use std::collections::BTreeMap;
use sugarsubstitute_update::MAX_METADATA_BYTES;
use tuf::{
    crypto::{HashAlgorithm, HashValue, calculate_hashes_from_slice},
    database::Database,
    metadata::{
        Metadata, MetadataDescription, MetadataPath, RawSignedMetadata, RootMetadata,
        SnapshotMetadata, TargetsMetadata, TimestampMetadata,
    },
    pouf::Pouf1,
    verify::verify_signatures,
};

/// Repository files by their flat published name.
pub type Files = BTreeMap<String, Vec<u8>>;

/// File name of the timestamp, the one metadata file replaced in place.
pub(crate) const TIMESTAMP: &str = "timestamp.json";

/// Published name of a versioned role file.
pub(crate) fn versioned(version: u32, role: &str) -> String {
    format!("{version}.{role}.json")
}

/// The authenticated current state of a published repository.
pub struct Repository {
    pub(crate) root: RootMetadata,
    pub(crate) timestamp: TimestampMetadata,
    pub(crate) snapshot: SnapshotMetadata,
    pub(crate) targets: TargetsMetadata,
}
impl Repository {
    /// Authenticate `files` from `trusted_root`, the root installations were shipped with.
    /// # Errors
    /// Rejects a missing or different copy of the trusted root, a broken root chain, invalid or
    /// insufficient signatures, and a snapshot or targets file that is not exactly the one its
    /// referencing role describes.
    pub fn authenticate(files: &Files, trusted_root: &[u8]) -> Result<Self, Error> {
        let mut database = Database::<Pouf1>::from_trusted_root(&raw(trusted_root)?)?;
        let first = versioned(database.trusted_root().version(), "root");
        if files.get(&first).map(Vec::as_slice) != Some(trusted_root) {
            return Err(invalid(format!("The repository does not publish the trusted {first}")));
        }
        loop {
            let next = database
                .trusted_root()
                .version()
                .checked_add(1)
                .ok_or_else(|| invalid("Root version space is exhausted"))?;
            let Some(bytes) = files.get(&versioned(next, "root")) else { break };
            database.update_root(&raw(bytes)?)?;
        }
        let root: RootMetadata = (**database.trusted_root()).clone();

        let timestamp_bytes = required(files, TIMESTAMP)?;
        let timestamp = verify_signatures(
            &MetadataPath::timestamp(),
            &raw::<TimestampMetadata>(timestamp_bytes)?,
            root.timestamp().threshold(),
            root.timestamp_keys(),
        )?;
        let snapshot_name = versioned(timestamp.snapshot().version(), "snapshot");
        let snapshot_bytes = required(files, &snapshot_name)?;
        bound(timestamp.snapshot(), snapshot_bytes, &snapshot_name)?;
        let snapshot = verify_signatures(
            &MetadataPath::snapshot(),
            &raw::<SnapshotMetadata>(snapshot_bytes)?,
            root.snapshot().threshold(),
            root.snapshot_keys(),
        )?;
        let reference = snapshot
            .meta()
            .get(&MetadataPath::targets())
            .ok_or_else(|| invalid(format!("{snapshot_name} does not reference targets")))?;
        let targets_name = versioned(reference.version(), "targets");
        let targets_bytes = required(files, &targets_name)?;
        bound(reference, targets_bytes, &targets_name)?;
        let targets = verify_signatures(
            &MetadataPath::targets(),
            &raw::<TargetsMetadata>(targets_bytes)?,
            root.targets().threshold(),
            root.targets_keys(),
        )?;
        let repository = Self {
            root,
            timestamp: (*timestamp).clone(),
            snapshot: (*snapshot).clone(),
            targets: (*targets).clone(),
        };
        if repository.snapshot.version() != timestamp.snapshot().version()
            || repository.targets.version() != reference.version()
        {
            return Err(invalid("A role's version differs from the version that references it"));
        }
        if !repository.targets.delegations().is_empty() {
            return Err(invalid("Delegated targets are not supported"));
        }
        Ok(repository)
    }

    /// The current root's version.
    pub fn root_version(&self) -> u32 {
        self.root.version()
    }

    /// The current timestamp's version.
    pub fn timestamp_version(&self) -> u32 {
        self.timestamp.version()
    }

    /// The current snapshot's version.
    pub fn snapshot_version(&self) -> u32 {
        self.snapshot.version()
    }

    /// The current targets' version.
    pub fn targets_version(&self) -> u32 {
        self.targets.version()
    }

    /// Names of the signed targets, in order.
    pub fn target_names(&self) -> Vec<String> {
        let mut names: Vec<String> =
            self.targets.targets().keys().map(|path| path.as_str().to_owned()).collect();
        names.sort();
        names
    }
}

/// Wrap untrusted role bytes after bounding them as the application does.
pub(crate) fn raw<M: Metadata>(bytes: &[u8]) -> Result<RawSignedMetadata<Pouf1, M>, Error> {
    if bytes.is_empty() || bytes.len() > MAX_METADATA_BYTES {
        return Err(invalid("A metadata file is empty or larger than installations accept"));
    }
    Ok(RawSignedMetadata::new(bytes.to_vec()))
}

fn required<'a>(files: &'a Files, name: &str) -> Result<&'a [u8], Error> {
    files
        .get(name)
        .map(Vec::as_slice)
        .ok_or_else(|| invalid(format!("The repository has no {name}")))
}

/// Require `bytes` to be exactly what `description` describes: length and every listed hash.
fn bound<M: Metadata>(
    description: &MetadataDescription<M>,
    bytes: &[u8],
    name: &str,
) -> Result<(), Error> {
    let mismatch = || invalid(format!("{name} is not the file its referencing role describes"));
    if description.length().is_some_and(|length| length != bytes.len()) {
        return Err(mismatch());
    }
    let algorithms: Vec<HashAlgorithm> = description.hashes().keys().cloned().collect();
    if algorithms.is_empty() || !algorithms.contains(&HashAlgorithm::Sha256) {
        return Err(invalid(format!("{name} is referenced without a SHA-256 digest")));
    }
    let computed: BTreeMap<String, HashValue> = calculate_hashes_from_slice(bytes, &algorithms)?
        .into_iter()
        .map(|(algorithm, value)| (format!("{algorithm:?}"), value))
        .collect();
    for (algorithm, expected) in description.hashes() {
        if computed.get(&format!("{algorithm:?}")) != Some(expected) {
            return Err(mismatch());
        }
    }
    Ok(())
}
