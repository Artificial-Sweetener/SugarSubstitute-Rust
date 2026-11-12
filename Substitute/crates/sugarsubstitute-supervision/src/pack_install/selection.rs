// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decide what one SugarCubes update check installs, without touching the installed pack.
//!
//! Order of work: prove the folder is a registry install this host may replace; find the
//! newest release across the Registry's version list and the GitHub release assets (the
//! Registry's copy when both have it); then prove no tracked file was changed locally,
//! offline from the pack's digest record or, for a pack ComfyUI Manager installed, against
//! the published release of the installed version. Both sources are always asked. A source
//! that does not answer is reported, and with nothing newer from the other the check fails
//! rather than calling the pack up to date.

use super::digest_record::FileDigests;
use super::error::Result;
use super::identity::ReleaseVersion;
use super::ownership::{
    InstalledDigests, NotOwnedReason, OwnedPack, PackOwnership, changed_tracked_files,
    inspect_pack_folder, installed_file_digests, read_digest_record,
};
use super::sources::{
    MetadataError, ReleaseOffer, ReleaseSourceConfig, ReleaseSourceKind, github_exact,
    github_latest, registry_exact, registry_latest,
};
use crate::durable_storage::pack_tree::PackTree;
use serde_json::Value;

/// A release source that did not produce a usable reply.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceFailure {
    /// The source answered that the page does not exist.
    NotFound,
    /// The source could not be reached, timed out, refused or answered unusably.
    Unavailable,
}

/// The network boundary an update check reads releases through.
///
/// Each call is one bounded request run by the execution owner; implementations never
/// retry on their own.
pub trait ReleaseSources {
    /// Fetch the JSON at one metadata URL.
    ///
    /// # Errors
    /// Returns why the source gave no usable reply.
    fn fetch_json(&self, url: &str) -> std::result::Result<Value, SourceFailure>;

    /// Download one offer's archive, verify its published digest and return each file's
    /// digest as extracted.
    ///
    /// # Errors
    /// Returns why the archive could not be obtained or verified.
    fn published_digests(
        &self,
        offer: &ReleaseOffer,
    ) -> std::result::Result<FileDigests, SourceFailure>;
}

/// What one update check decided.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdateDecision {
    /// The folder is not one this host may replace.
    NotManaged(NotOwnedReason),
    /// No newer release is published on any source that answered.
    UpToDate {
        /// Sources that did not answer.
        unchecked: Vec<ReleaseSourceKind>,
    },
    /// Install this release over the unchanged installed files.
    Install {
        /// The release to install.
        offer: ReleaseOffer,
        /// The installed version it replaces.
        from: ReleaseVersion,
        /// The tracked files' digests the plan was made against.
        installed: InstalledDigests,
        /// Sources that did not answer.
        unchecked: Vec<ReleaseSourceKind>,
    },
    /// The check could not finish; the installed pack stays as it is.
    CheckFailed {
        /// Sources that did not answer.
        unchecked: Vec<ReleaseSourceKind>,
    },
}

/// Decide what to do for the pack folder ComfyUI loads, `pack_id` naming the pack.
pub(crate) fn plan_update(
    folder: &PackTree,
    config: &ReleaseSourceConfig,
    sources: &dyn ReleaseSources,
) -> Result<UpdateDecision> {
    let pack = match inspect_pack_folder(folder)? {
        PackOwnership::NotOwned(reason) => return Ok(UpdateDecision::NotManaged(reason)),
        PackOwnership::Owned(pack) if !pack.identity.is_pack(&config.pack_id) => {
            return Ok(UpdateDecision::NotManaged(NotOwnedReason::NotRegistryInstall));
        }
        PackOwnership::Owned(pack) => pack,
    };
    let current = pack.identity.version;
    let (offer, unchecked) = newest_offer(config, sources, current);
    let Some(offer) = offer else {
        return Ok(if unchecked.is_empty() {
            UpdateDecision::UpToDate { unchecked }
        } else {
            UpdateDecision::CheckFailed { unchecked }
        });
    };
    let installed = installed_file_digests(folder, &pack)?;
    let Some(expected) = expected_digests(folder, &pack, config, sources, &installed)? else {
        return Ok(UpdateDecision::CheckFailed { unchecked });
    };
    if !changed_tracked_files(&installed, &expected).is_empty() {
        return Ok(UpdateDecision::NotManaged(NotOwnedReason::ChangedFiles));
    }
    Ok(UpdateDecision::Install { offer, from: current, installed, unchecked })
}

/// Return the newest offer above `current` and the sources that did not answer.
pub fn newest_offer(
    config: &ReleaseSourceConfig,
    sources: &dyn ReleaseSources,
    current: ReleaseVersion,
) -> (Option<ReleaseOffer>, Vec<ReleaseSourceKind>) {
    let asks: [(
        ReleaseSourceKind,
        String,
        fn(
            &ReleaseSourceConfig,
            &Value,
        ) -> std::result::Result<Option<ReleaseOffer>, MetadataError>,
    ); 2] = [
        (ReleaseSourceKind::Registry, config.registry_versions_url(), registry_latest),
        (ReleaseSourceKind::GitHubRelease, config.github_releases_url(), github_latest),
    ];
    let mut offers = Vec::new();
    let mut unchecked = Vec::new();
    for (kind, url, read) in asks {
        match sources
            .fetch_json(&url)
            .map_err(|_| ())
            .and_then(|reply| read(config, &reply).map_err(|_| ()))
        {
            Ok(Some(offer)) if offer.version > current => offers.push(offer),
            Ok(_) => {}
            Err(()) => {
                tracing::warn!(
                    target: "sugarsubstitute::pack_install",
                    operation = "check_source",
                    source = kind.as_str(),
                    "A SugarCubes release source did not answer"
                );
                unchecked.push(kind);
            }
        }
    }
    // Offers are in source order, so the Registry wins a tie.
    let newest = offers.iter().map(|offer| offer.version).max();
    let offer = offers.into_iter().find(|offer| Some(offer.version) == newest);
    (offer, unchecked)
}

/// Return what each tracked file should hold, offline when the digest record applies.
fn expected_digests(
    folder: &PackTree,
    pack: &OwnedPack,
    config: &ReleaseSourceConfig,
    sources: &dyn ReleaseSources,
    installed: &InstalledDigests,
) -> Result<Option<FileDigests>> {
    let version = pack.identity.version.to_string();
    if let Some(recorded) = read_digest_record(folder, &version, &pack.entries)? {
        return Ok(Some(recorded));
    }
    let Some(published) = published_digests(config, sources, pack.identity.version) else {
        return Ok(None);
    };
    let matches = installed
        .iter()
        .all(|(path, digest)| digest.is_some() && published.get(path) == digest.as_ref());
    if matches {
        let adopted: FileDigests = installed
            .keys()
            .filter_map(|path| Some((path.clone(), published.get(path)?.clone())))
            .collect();
        let tracking =
            folder.read(super::entry::TRACKING_FILE_NAME, super::tracking::MAX_TRACKING_BYTES)?;
        super::ownership::write_digest_record(
            folder,
            &version,
            &super::ownership::sha256_hex(&tracking),
            &adopted,
        )?;
        tracing::info!(
            target: "sugarsubstitute::pack_install",
            operation = "adopt_digest_record",
            version = %version,
            "Recorded the digests of a pack installed by another installer"
        );
    }
    Ok(Some(published))
}

fn published_digests(
    config: &ReleaseSourceConfig,
    sources: &dyn ReleaseSources,
    version: ReleaseVersion,
) -> Option<FileDigests> {
    let registry = || {
        let reply = sources.fetch_json(&config.registry_install_url(version)).ok()?;
        registry_exact(config, &reply, version, false).ok().flatten()
    };
    let github = || {
        let reply = sources.fetch_json(&config.github_release_url(version)).ok()?;
        github_exact(config, &reply, version).ok().flatten()
    };
    let offer = registry().or_else(github)?;
    sources.published_digests(&offer).ok()
}
