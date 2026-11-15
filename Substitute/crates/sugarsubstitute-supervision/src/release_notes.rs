// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Read a component's published release notes from its changelog at its release tag.
//!
//! The changelog semantic-release commits with each release is read through the GitHub contents
//! API at that release's tag, over the host's update HTTP owner: one bounded request on the
//! admitted worker whose `context` supplies the deadline and cancellation, with the update
//! idle timeout. Nothing retries on its own. The reply grants no authority; it is text shown to
//! the user, parsed into version sections and bounded before it leaves this owner.

pub mod changelog;
#[cfg(feature = "local-update-qualification")]
mod qualification;

#[cfg(test)]
mod tests;

use crate::update_http::{Failure, Http};
use base64::Engine;
use serde::Deserialize;
use std::time::Duration;
use sugarsubstitute_contracts::{execution::JobContext, host_release_notes::ReleaseNotesFailure};
use sugarsubstitute_update::http::{Policy, Rejection};

pub use changelog::{MAX_NOTES_BYTES, Selection};

/// The longest the changelog request may sit without progress before it is abandoned.
pub const RELEASE_NOTES_IDLE: Duration = Duration::from_secs(20);
/// The longest one fetch may take overall, its request and parsing together.
pub const RELEASE_NOTES_DEADLINE: Duration = Duration::from_secs(60);
/// Largest contents API reply read: the API serves files of up to 1 MiB inline, as base64.
pub const MAX_REPLY_BYTES: u64 = 2 * 1024 * 1024;
/// Admission one fetch needs beyond the HTTP owner's own: the reply, the decoded changelog and
/// the selected notes.
pub const RELEASE_NOTES_RESERVATION_BYTES: u64 =
    crate::update_http::RESERVATION_BYTES + 3 * MAX_REPLY_BYTES;

/// Where a component's changelog is published.
pub struct ChangelogSource {
    policy: Policy,
    api_root: String,
    repository: &'static str,
    tag_prefix: &'static str,
    path: &'static str,
    idle: Duration,
}

impl ChangelogSource {
    /// SugarSubstitute's changelog in its release repository.
    ///
    /// A local-update qualification build reads it from the loopback fixture named by
    /// `QUALIFICATION_RELEASE_NOTES_PORT` when that variable holds a port, so a
    /// proof never reaches GitHub.
    ///
    /// # Errors
    /// Refuses an origin the update HTTP policy cannot admit, which would be a defect here.
    pub fn substitute() -> Result<Self, Rejection> {
        #[cfg(feature = "local-update-qualification")]
        if let Some(port) = qualification::loopback_port() {
            return Self::loopback(port);
        }
        Self::published()
    }

    /// The changelog in SugarSubstitute's published release repository.
    fn published() -> Result<Self, Rejection> {
        Ok(Self {
            policy: Policy::https("https://api.github.com/", &[])?,
            api_root: "https://api.github.com".into(),
            repository: "Artificial-Sweetener/SugarSubstitute-Rust",
            tag_prefix: "substitute-v",
            path: "Substitute/CHANGELOG.md",
            idle: RELEASE_NOTES_IDLE,
        })
    }

    /// The same changelog served by a literal loopback fixture on `port`.
    #[cfg(any(test, feature = "local-update-qualification"))]
    pub(crate) fn loopback(port: u16) -> Result<Self, Rejection> {
        Ok(Self {
            policy: Policy::loopback(port)?,
            api_root: format!("http://127.0.0.1:{port}"),
            ..Self::published()?
        })
    }

    /// The same source abandoning a request after idle without progress.
    #[cfg(test)]
    pub(crate) fn with_idle(self, idle: Duration) -> Self {
        Self { idle, ..self }
    }

    /// The contents API address of the changelog at the release tag of `version`.
    fn address(&self, version: &str) -> String {
        format!(
            "{}/repos/{}/contents/{}?ref={}{version}",
            self.api_root, self.repository, self.path, self.tag_prefix
        )
    }
}

/// The contents API's description of one file.
#[derive(Deserialize)]
struct Contents {
    encoding: String,
    content: String,
}

/// Fetch the published notes of every version after `since` through `running`, newest first.
///
/// `running` and `since` are release versions; a `since` that is not older than `running`
/// asks for `running` alone.
///
/// # Errors
/// [`ReleaseNotesFailure::NotPublished`] when the release tag or its changelog section does not
/// exist; [`ReleaseNotesFailure::Unavailable`] for every other failure, including the deadline,
/// cancellation, an oversized reply and a reply that is not a readable changelog.
pub fn fetch(
    http: &mut Http,
    source: &ChangelogSource,
    running: &str,
    since: &str,
    context: &JobContext,
) -> Result<Selection, ReleaseNotesFailure> {
    let endpoint =
        source.policy.endpoint(&source.address(running)).map_err(|_| unavailable("address"))?;
    let mut reply = Vec::new();
    http.file(&source.policy, endpoint, MAX_REPLY_BYTES, source.idle, context, &mut reply)
        .map_err(|failure| match failure {
            Failure::Policy(Rejection::Status(404)) => ReleaseNotesFailure::NotPublished,
            failure => {
                tracing::warn!(operation = "release_notes_request", error = %failure,
                    "The release notes could not be read");
                ReleaseNotesFailure::Unavailable
            }
        })?;
    let contents: Contents = serde_json::from_slice(&reply).map_err(|_| unavailable("reply"))?;
    if contents.encoding != "base64" {
        return Err(unavailable("encoding"));
    }
    let packed: Vec<u8> =
        contents.content.bytes().filter(|byte| !byte.is_ascii_whitespace()).collect();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(packed)
        .map_err(|_| unavailable("content"))?;
    let changelog = String::from_utf8(bytes).map_err(|_| unavailable("text"))?;
    changelog::select(&changelog, running, since).ok_or(ReleaseNotesFailure::NotPublished)
}

/// A reply this owner cannot read, logged by what was wrong and never by its content.
fn unavailable(part: &'static str) -> ReleaseNotesFailure {
    tracing::warn!(
        operation = "release_notes_reply",
        part,
        "The published release notes could not be read"
    );
    ReleaseNotesFailure::Unavailable
}
