// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Check a repository with the application's own verifier, exactly as an installation does.
//!
//! Discovery follows the application's order: every next root, then the timestamp, the snapshot
//! it names and the targets that snapshot names, through [RepositoryTrust]. With a previous
//! state, the installation first accepts that state and keeps its checkpoint across a restart,
//! so a new state that rolls back or rewrites an accepted version is refused here, before it
//! is published. Every signed channel descriptor must then yield an authenticated offer whose
//! package is signed too, and every package supplied is streamed through its signed digest.
use crate::{
    Error, Files, invalid,
    repository::{TIMESTAMP, raw, versioned},
    sign::{descriptor_routes, seconds},
};
use sugarsubstitute_update::{MetadataSet, RepositoryTrust};
use tuf::metadata::{Metadata, RootMetadata, SnapshotMetadata, TargetsMetadata, TimestampMetadata};

/// Package bytes, streamed in chunks; `false` when a package is not supplied.
pub trait Payloads {
    /// Feed every byte of `name` to `sink`, in order.
    /// # Errors
    /// Returns read failures and the sink's own errors.
    fn stream(
        &self,
        name: &str,
        sink: &mut dyn FnMut(&[u8]) -> Result<(), Error>,
    ) -> Result<bool, Error>;
}

/// What the application accepted, with each role's version and expiry (Unix UTC seconds).
#[derive(Debug, Eq, PartialEq)]
pub struct Report {
    /// Root version and expiry.
    pub root: (u32, i64),
    /// Timestamp version and expiry.
    pub timestamp: (u32, i64),
    /// Snapshot version and expiry.
    pub snapshot: (u32, i64),
    /// Targets version and expiry.
    pub targets: (u32, i64),
    /// Channel descriptors that produced an authenticated offer.
    pub offers: Vec<String>,
    /// Packages whose supplied bytes matched their signed length and digest.
    pub packages: Vec<String>,
}

/// Verify `files` at `now` (Unix UTC seconds) as an installation shipped with `trusted_root`;
/// with `previous`, as one that had already accepted that earlier state.
/// # Errors
/// Returns the application's own rejection, or names a signed descriptor that is not published.
pub fn verify(
    trusted_root: &[u8],
    previous: Option<&Files>,
    files: &Files,
    payloads: &dyn Payloads,
    now: i64,
) -> Result<Report, Error> {
    let mut trust = RepositoryTrust::bootstrap(trusted_root)?;
    if let Some(previous) = previous {
        // The installation saw the earlier state while it was valid, never later than `now`.
        let seen = now.min(earliest_expiry(previous)?.saturating_sub(1));
        discover(&mut trust, previous, seen)?;
        trust = RepositoryTrust::from_trusted_checkpoint(&trust.checkpoint()?)?;
    }
    let targets = discover(&mut trust, files, now)?;
    let mut offers = Vec::new();
    let mut packages = Vec::new();
    let names: Vec<String> =
        targets.targets().keys().map(|path| path.as_str().to_owned()).collect();
    for request in descriptor_routes() {
        let name = request.target_name();
        if !names.contains(&name) {
            continue;
        }
        let bytes = files
            .get(&name)
            .ok_or_else(|| invalid(format!("{name} is signed but not published")))?;
        let offer = trust.offer(&request, bytes, now)?;
        offers.push(name);
        let package = offer.offer().package_name();
        let target = trust.target(&package, now)?;
        let mut stream = target.verifier(now)?;
        let supplied = payloads.stream(&package, &mut |chunk| Ok(stream.push(chunk)?))?;
        if supplied {
            stream.finish(now)?;
            packages.push(package);
        }
    }
    let current = trust
        .next_root_version()?
        .checked_sub(1)
        .ok_or_else(|| invalid("The verifier holds no root"))?;
    let root = parse::<RootMetadata>(files, &versioned(current, "root"))?;
    let timestamp = parse::<TimestampMetadata>(files, TIMESTAMP)?;
    let snapshot =
        parse::<SnapshotMetadata>(files, &versioned(timestamp.snapshot().version(), "snapshot"))?;
    Ok(Report {
        root: (root.version(), seconds(root.expires())),
        timestamp: (timestamp.version(), seconds(timestamp.expires())),
        snapshot: (snapshot.version(), seconds(snapshot.expires())),
        targets: (targets.version(), seconds(targets.expires())),
        offers,
        packages,
    })
}

/// Fetch and authenticate a complete metadata set in the application's order.
fn discover(
    trust: &mut RepositoryTrust,
    files: &Files,
    now: i64,
) -> Result<TargetsMetadata, Error> {
    while let Some(root) = files.get(&versioned(trust.next_root_version()?, "root")) {
        trust.rotate(root)?;
    }
    let timestamp = required(files, TIMESTAMP)?;
    let snapshot_version = trust.snapshot_version(timestamp, now)?;
    let snapshot = required(files, &versioned(snapshot_version, "snapshot"))?;
    let targets_version = trust.targets_version(timestamp, snapshot, now)?;
    let name = versioned(targets_version, "targets");
    let targets = required(files, &name)?;
    trust.refresh(MetadataSet { timestamp, snapshot, targets }, now)?;
    // The bytes are authenticated now; parsing them only lists what they sign.
    parse::<TargetsMetadata>(files, &name)
}

/// The earliest expiry among the roles of a state, read without authenticating it: it only
/// chooses when the verifier, which authenticates everything, looks at that state.
fn earliest_expiry(files: &Files) -> Result<i64, Error> {
    let timestamp = parse::<TimestampMetadata>(files, TIMESTAMP)?;
    let snapshot =
        parse::<SnapshotMetadata>(files, &versioned(timestamp.snapshot().version(), "snapshot"))?;
    let targets = snapshot
        .meta()
        .values()
        .map(|description| description.version())
        .max()
        .ok_or_else(|| invalid("The snapshot references no targets"))?;
    let targets = parse::<TargetsMetadata>(files, &versioned(targets, "targets"))?;
    // Installations check only the newest root's expiry; superseded roots may have expired.
    let mut newest = None;
    let mut version: u32 = 1;
    while let Some(bytes) = files.get(&versioned(version, "root")) {
        newest = Some(bytes);
        version =
            version.checked_add(1).ok_or_else(|| invalid("Root version space is exhausted"))?;
    }
    let root = raw::<RootMetadata>(newest.ok_or_else(|| invalid("The state has no root"))?)?
        .parse_untrusted()?
        .assume_valid()?;
    let expiries = [
        seconds(timestamp.expires()),
        seconds(snapshot.expires()),
        seconds(targets.expires()),
        seconds(root.expires()),
    ];
    expiries.into_iter().min().ok_or_else(|| invalid("The state has no roles"))
}

fn parse<M: Metadata>(files: &Files, name: &str) -> Result<M, Error> {
    Ok(raw::<M>(required(files, name)?)?.parse_untrusted()?.assume_valid()?)
}

fn required<'a>(files: &'a Files, name: &str) -> Result<&'a [u8], Error> {
    files
        .get(name)
        .map(Vec::as_slice)
        .ok_or_else(|| invalid(format!("The repository has no {name}")))
}
