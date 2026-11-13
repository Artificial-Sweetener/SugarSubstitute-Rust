// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Sign the next repository state with the lifetimes in [crate::lifetimes].
//!
//! Every change produces new versioned files and a new `timestamp.json`; existing versioned
//! files are never rewritten, so an installation never sees two different files under one
//! version. Each key must be the one the root in effect authorizes for its role, so a change
//! never publishes metadata installations would refuse.
use crate::{
    Error, Files, Repository, invalid,
    keys::SigningKey,
    lifetimes::{self, expiry},
    repository::{TIMESTAMP, versioned},
};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use sugarsubstitute_contracts::update::{Channel, Component, Platform, Request};
use tuf::{
    crypto::{HashAlgorithm, HashValue, PublicKey},
    metadata::{
        MetadataDescription, MetadataPath, RootMetadata, RootMetadataBuilder, SnapshotMetadata,
        SnapshotMetadataBuilder, TargetDescription, TargetPath, TargetsMetadataBuilder,
        TimestampMetadataBuilder,
    },
    pouf::Pouf1,
};

/// The keys CI holds for the roles it re-signs routinely.
pub struct OnlineKeys<'a> {
    /// Targets role key.
    pub targets: &'a SigningKey,
    /// Snapshot role key.
    pub snapshot: &'a SigningKey,
    /// Timestamp role key.
    pub timestamp: &'a SigningKey,
}

/// One file to publish as a signed target, described by its exact length and SHA-256 digest.
pub struct NewTarget {
    /// Flat published name: a channel descriptor or `<sha256>.ssupdate`.
    pub name: String,
    /// Exact length in bytes.
    pub length: u64,
    /// SHA-256 of the exact bytes.
    pub sha256: [u8; 32],
    /// The bytes of a channel descriptor, which is published with the metadata; packages are
    /// published separately and carry none.
    pub descriptor: Option<Vec<u8>>,
}

/// What to sign next.
pub enum Change<'a> {
    /// Re-sign only the timestamp, for the same snapshot (daily).
    RefreshTimestamp {
        /// Timestamp role key.
        timestamp: &'a SigningKey,
    },
    /// Re-sign targets, snapshot and timestamp with unchanged targets (monthly).
    Resign(OnlineKeys<'a>),
    /// Add or replace targets, then re-sign snapshot and timestamp (each release).
    Release {
        /// Online role keys.
        keys: OnlineKeys<'a>,
        /// Targets to add or replace by name.
        targets: Vec<NewTarget>,
    },
    /// Publish the next root, signed by the current and next root keys, naming the online keys
    /// given here, then re-sign every online role under it (yearly, or to replace a key).
    RotateRoot {
        /// A key the current root authorizes for the root role.
        current_root: &'a SigningKey,
        /// The root key of the next root; may equal `current_root` to extend its lifetime.
        next_root: &'a SigningKey,
        /// Online role keys the next root authorizes.
        keys: OnlineKeys<'a>,
    },
}

/// New files to publish, in this order: versioned metadata, replaced channel descriptors,
/// then `timestamp.json`, which authorizes the rest and is the file installations read first.
pub struct Published {
    /// New versioned metadata and `timestamp.json`, by published name.
    pub files: Files,
    /// Channel descriptors this change signs, which replace their earlier versions.
    pub descriptors: Files,
}

/// Create a repository: root 1, empty targets 1, snapshot 1 and timestamp 1.
/// # Errors
/// Rejects an unrepresentable time and signing failures.
pub fn initialize(root: &SigningKey, keys: &OnlineKeys<'_>, now: i64) -> Result<Published, Error> {
    let mut files = Files::new();
    let root_metadata =
        root_builder(1, root.public(), keys, now)?.signed::<Pouf1>(root.signer())?;
    files.insert(versioned(1, "root"), root_metadata.to_raw()?.as_bytes().to_vec());
    online(&mut files, keys, [1, 1, 1], HashMap::new(), now)?;
    Ok(Published { files, descriptors: Files::new() })
}

impl Repository {
    /// Sign the change on top of this authenticated state at `now` (Unix UTC seconds).
    /// # Errors
    /// Rejects keys the root in effect does not authorize, invalid target names, exhausted
    /// version space and signing failures.
    pub fn sign(&self, change: Change<'_>, now: i64) -> Result<Published, Error> {
        let mut files = Files::new();
        let mut descriptors = Files::new();
        let versions = |step: u32| -> Result<[u32; 3], Error> {
            let next = |version: u32| {
                version.checked_add(step).ok_or_else(|| invalid("Version space is exhausted"))
            };
            Ok([
                next(self.timestamp_version())?,
                next(self.snapshot_version())?,
                next(self.targets_version())?,
            ])
        };
        match change {
            Change::RefreshTimestamp { timestamp } => {
                authorize(&self.root, Role::Timestamp, timestamp)?;
                let [version, ..] = versions(1)?;
                let bytes = TimestampMetadataBuilder::from_metadata_description(
                    self.timestamp.snapshot().clone(),
                )
                .version(version)
                .expires(expiry(now, lifetimes::TIMESTAMP_SECONDS)?)
                .signed::<Pouf1>(timestamp.signer())?
                .to_raw()?;
                files.insert(TIMESTAMP.into(), bytes.as_bytes().to_vec());
            }
            Change::Resign(keys) => {
                authorize_online(&self.root, &keys)?;
                online(&mut files, &keys, versions(1)?, self.current_targets(), now)?;
            }
            Change::Release { keys, targets } => {
                authorize_online(&self.root, &keys)?;
                let mut descriptions = self.current_targets();
                for target in targets {
                    let (path, description) = described(&target)?;
                    descriptions.insert(path, description);
                    if let Some(bytes) = target.descriptor {
                        descriptors.insert(target.name, bytes);
                    }
                }
                online(&mut files, &keys, versions(1)?, descriptions, now)?;
            }
            Change::RotateRoot { current_root, next_root, keys } => {
                authorize(&self.root, Role::Root, current_root)?;
                let version = self
                    .root_version()
                    .checked_add(1)
                    .ok_or_else(|| invalid("Root version space is exhausted"))?;
                let mut root = root_builder(version, next_root.public(), &keys, now)?
                    .signed::<Pouf1>(next_root.signer())?;
                if current_root.public().key_id() != next_root.public().key_id() {
                    root.add_signature(current_root.signer())?;
                }
                files.insert(versioned(version, "root"), root.to_raw()?.as_bytes().to_vec());
                online(&mut files, &keys, versions(1)?, self.current_targets(), now)?;
            }
        }
        Ok(Published { files, descriptors })
    }

    fn current_targets(&self) -> HashMap<TargetPath, TargetDescription> {
        self.targets.targets().clone()
    }
}

/// A root with one key and threshold one per role, and consistent snapshots, as installations
/// require.
fn root_builder(
    version: u32,
    root: &PublicKey,
    keys: &OnlineKeys<'_>,
    now: i64,
) -> Result<RootMetadataBuilder, Error> {
    Ok(RootMetadataBuilder::new()
        .version(version)
        .expires(expiry(now, lifetimes::ROOT_SECONDS)?)
        .consistent_snapshot(true)
        .root_key(root.clone())
        .targets_key(keys.targets.public().clone())
        .snapshot_key(keys.snapshot.public().clone())
        .timestamp_key(keys.timestamp.public().clone()))
}

/// Sign targets, the snapshot that binds them and the timestamp that binds it.
fn online(
    files: &mut Files,
    keys: &OnlineKeys<'_>,
    [timestamp_version, snapshot_version, targets_version]: [u32; 3],
    targets: HashMap<TargetPath, TargetDescription>,
    now: i64,
) -> Result<(), Error> {
    let mut builder = TargetsMetadataBuilder::new()
        .version(targets_version)
        .expires(expiry(now, lifetimes::TARGETS_SECONDS)?);
    for (path, description) in targets {
        builder = builder.insert_target_description(path, description);
    }
    let targets = builder.signed::<Pouf1>(keys.targets.signer())?.to_raw()?.as_bytes().to_vec();
    let snapshot = SnapshotMetadataBuilder::new()
        .version(snapshot_version)
        .expires(expiry(now, lifetimes::SNAPSHOT_SECONDS)?)
        .insert_metadata_description(
            MetadataPath::targets(),
            description(&targets, targets_version)?,
        )
        .signed::<Pouf1>(keys.snapshot.signer())?
        .to_raw()?
        .as_bytes()
        .to_vec();
    let timestamp =
        TimestampMetadataBuilder::from_metadata_description(description::<SnapshotMetadata>(
            &snapshot,
            snapshot_version,
        )?)
        .version(timestamp_version)
        .expires(expiry(now, lifetimes::TIMESTAMP_SECONDS)?)
        .signed::<Pouf1>(keys.timestamp.signer())?
        .to_raw()?
        .as_bytes()
        .to_vec();
    files.insert(versioned(targets_version, "targets"), targets);
    files.insert(versioned(snapshot_version, "snapshot"), snapshot);
    files.insert(TIMESTAMP.into(), timestamp);
    Ok(())
}

fn description<M: tuf::metadata::Metadata>(
    bytes: &[u8],
    version: u32,
) -> Result<MetadataDescription<M>, Error> {
    Ok(MetadataDescription::from_slice(bytes, version, &[HashAlgorithm::Sha256])?)
}

/// Describe a new target, accepting only names installations request.
fn described(target: &NewTarget) -> Result<(TargetPath, TargetDescription), Error> {
    let digest: String = target.sha256.iter().map(|byte| format!("{byte:02x}")).collect();
    let package = format!("{digest}.ssupdate");
    if target.name.ends_with(".ssupdate") && target.name != package {
        return Err(invalid(format!("{} is not named by its SHA-256 digest", target.name)));
    }
    if target.name != package && !descriptor_names().contains(&target.name) {
        return Err(invalid(format!(
            "{} is neither a channel descriptor nor a package installations request",
            target.name
        )));
    }
    if target.length == 0 {
        return Err(invalid(format!("{} is empty", target.name)));
    }
    let is_descriptor = target.name != package;
    let carries = target.descriptor.as_ref().map(|bytes| bytes.len() as u64);
    if is_descriptor != carries.is_some() || carries.is_some_and(|length| length != target.length) {
        return Err(invalid(format!("{} must carry exactly its descriptor bytes", target.name)));
    }
    let hashes = HashMap::from([(HashAlgorithm::Sha256, HashValue::new(target.sha256.to_vec()))]);
    Ok((
        TargetPath::new(target.name.clone())?,
        TargetDescription::new(target.length, hashes, HashMap::new())?,
    ))
}

/// Every channel descriptor name an installation can request, from the contract's own naming.
pub(crate) fn descriptor_names() -> Vec<String> {
    descriptor_routes().into_iter().map(|request| request.target_name()).collect()
}

/// One local request per route; the routing fields name the descriptor, the rest are neutral.
pub(crate) fn descriptor_routes() -> Vec<Request> {
    let mut requests = Vec::new();
    for component in [Component::Application, Component::Updater] {
        for channel in [Channel::Release, Channel::Canary] {
            for platform in [
                Platform::WindowsX86_64,
                Platform::WindowsAarch64,
                Platform::MacosX86_64,
                Platform::MacosAarch64,
                Platform::LinuxX86_64,
                Platform::LinuxAarch64,
            ] {
                requests.push(Request {
                    channel,
                    platform,
                    component,
                    sequence_floor: 0,
                    sequence_digest: None,
                    updater_release: u64::MAX,
                    updater_protocol: 1,
                    package_schema: 1,
                    intent: sugarsubstitute_contracts::installation::PreparationIntent::Automatic,
                });
            }
        }
    }
    requests
}

#[derive(Clone, Copy)]
enum Role {
    Root,
    Targets,
    Snapshot,
    Timestamp,
}

fn authorize_online(root: &RootMetadata, keys: &OnlineKeys<'_>) -> Result<(), Error> {
    authorize(root, Role::Targets, keys.targets)?;
    authorize(root, Role::Snapshot, keys.snapshot)?;
    authorize(root, Role::Timestamp, keys.timestamp)
}

/// Require the key to be one the root authorizes for the role.
fn authorize(root: &RootMetadata, role: Role, key: &SigningKey) -> Result<(), Error> {
    let (name, ids) = match role {
        Role::Root => ("root", root.root().key_ids()),
        Role::Targets => ("targets", root.targets().key_ids()),
        Role::Snapshot => ("snapshot", root.snapshot().key_ids()),
        Role::Timestamp => ("timestamp", root.timestamp().key_ids()),
    };
    if !ids.contains(key.public().key_id()) {
        return Err(invalid(format!(
            "The {name} key given is not the one root version {} authorizes",
            tuf::metadata::Metadata::version(root)
        )));
    }
    Ok(())
}

/// Expiry of a role, for reports.
pub(crate) fn seconds(time: &DateTime<Utc>) -> i64 {
    time.timestamp()
}
