// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Read SugarCubes' release sources over the host's update HTTP owner.
//!
//! Every request goes to an exact URL derived from the pack's own release description and
//! stays on that source's configured origins, redirects included: the Registry API and its
//! archive CDN, or the GitHub API and its download hosts. Replies and archives are bounded;
//! nothing here grants them authenticity, which the decisions and the archive digest check
//! establish. Each call is one bounded request on the admitted worker whose `context` this
//! owner borrows; nothing retries on its own.

use super::archive::{ArchiveLimits, digest_registry_source};
use super::digest_record::FileDigests;
use super::selection::{ReleaseSources, SourceFailure};
use super::sources::{MAX_ARCHIVE_BYTES, MetadataError, ReleaseOffer, ReleaseSourceConfig};
use super::startup::ReleaseDownloads;
use crate::durable_storage::pack_tree::PackTree;
use crate::update_http::{Failure, Http};
use serde_json::Value;
use std::{cell::RefCell, io, time::Duration};
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_update::{
    MAX_METADATA_BYTES,
    http::{Policy, Rejection},
};

const PUBLISHED_ARCHIVE: &str = "published.zip";

/// The longest a release request may sit without progress before it is abandoned.
pub const RELEASE_REQUEST_IDLE: Duration = Duration::from_secs(30);

/// The release sources of one pack, reached through one HTTP owner on one admitted worker.
pub struct HttpReleaseSources<'a> {
    http: RefCell<&'a mut Http>,
    context: &'a JobContext,
    policies: [Policy; 2],
    scratch: PackTree,
}

impl<'a> HttpReleaseSources<'a> {
    /// Reach `config`'s Registry and GitHub origins over HTTPS through `http`, keeping any
    /// archive downloaded only for comparison in `scratch`, a folder in the host's storage.
    ///
    /// # Errors
    /// Refuses an origin that is not HTTPS.
    pub fn new(
        http: &'a mut Http,
        config: &ReleaseSourceConfig,
        scratch: &std::path::Path,
        context: &'a JobContext,
    ) -> Result<Self, MetadataError> {
        let refused = |_| MetadataError("The release-source configuration is not HTTPS.");
        let hosts: Vec<String> =
            config.download_hosts.iter().map(|host| format!("https://{host}/")).collect();
        let hosts: Vec<&str> = hosts.iter().map(String::as_str).collect();
        let registry = Policy::https(
            &format!("{}/", config.registry_api_root),
            &[&format!("https://{}/", config.registry_archive_host)],
        )
        .map_err(refused)?;
        let github =
            Policy::https(&format!("{}/", config.github_api_root), &hosts).map_err(refused)?;
        Self::with_policies(http, [registry, github], scratch, context)
            .map_err(|_| MetadataError("The comparison download folder is invalid."))
    }

    /// Reach the sources through exactly `policies`; tests serve both from one loopback origin.
    pub(crate) fn with_policies(
        http: &'a mut Http,
        policies: [Policy; 2],
        scratch: &std::path::Path,
        context: &'a JobContext,
    ) -> io::Result<Self> {
        Ok(Self { http: RefCell::new(http), context, policies, scratch: PackTree::at(scratch)? })
    }

    fn fetch(
        &self,
        url: &str,
        output: &mut dyn io::Write,
        maximum: u64,
    ) -> Result<u64, SourceFailure> {
        let (policy, endpoint) = self
            .policies
            .iter()
            .find_map(|policy| Some((policy, policy.endpoint(url).ok()?)))
            .ok_or(SourceFailure::Unavailable)?;
        let mut http = self.http.borrow_mut();
        http.file(policy, endpoint, maximum, RELEASE_REQUEST_IDLE, self.context, output).map_err(
            |failure| {
                tracing::warn!(
                    target: "sugarsubstitute::pack_install",
                    operation = "release_request",
                    error = %failure,
                    "A SugarCubes release request failed"
                );
                match failure {
                    Failure::Policy(Rejection::Status(404)) => SourceFailure::NotFound,
                    _ => SourceFailure::Unavailable,
                }
            },
        )
    }
}

impl ReleaseSources for HttpReleaseSources<'_> {
    fn fetch_json(&self, url: &str) -> Result<Value, SourceFailure> {
        let mut bytes = Vec::new();
        self.fetch(url, &mut bytes, MAX_METADATA_BYTES as u64)?;
        serde_json::from_slice(&bytes).map_err(|_| SourceFailure::Unavailable)
    }

    fn published_digests(&self, offer: &ReleaseOffer) -> Result<FileDigests, SourceFailure> {
        let digests = (|| {
            self.scratch.create().map_err(|_| SourceFailure::Unavailable)?;
            let mut refused = None;
            let written = self.scratch.replace_with(PUBLISHED_ARCHIVE, |output| {
                self.download(offer, output).map_err(|failure| {
                    refused = Some(failure);
                    io::Error::other("The comparison download failed")
                })
            });
            if let Some(failure) = refused {
                return Err(failure);
            }
            written.map_err(|_| SourceFailure::Unavailable)?;
            let file =
                self.scratch.open(PUBLISHED_ARCHIVE).map_err(|_| SourceFailure::Unavailable)?;
            digest_registry_source(file, ArchiveLimits::default())
                .map_err(|_| SourceFailure::Unavailable)
        })();
        if let Err(cause) = self.scratch.remove_file(PUBLISHED_ARCHIVE) {
            tracing::warn!(
                target: "sugarsubstitute::pack_install",
                operation = "discard_download",
                error = %cause,
                "A downloaded SugarCubes archive could not be removed"
            );
        }
        digests
    }
}

impl ReleaseDownloads for HttpReleaseSources<'_> {
    fn download(
        &self,
        offer: &ReleaseOffer,
        output: &mut dyn io::Write,
    ) -> Result<(), SourceFailure> {
        let host = url::Url::parse(&offer.archive_url)
            .ok()
            .and_then(|url| url.host_str().map(str::to_owned));
        if !host.is_some_and(|host| offer.allowed_hosts.contains(&host)) {
            return Err(SourceFailure::Unavailable);
        }
        self.fetch(&offer.archive_url, output, MAX_ARCHIVE_BYTES).map(|_| ())
    }
}
