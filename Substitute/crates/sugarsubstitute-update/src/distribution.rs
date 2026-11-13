// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Producing-build repository binding, transport policy, update policy and release-notes location,
//! independent of signed remote offers.
use crate::http::Policy;
use serde::Deserialize;
use std::io;

/// Maximum encoded producing-build configuration, in bytes.
pub const MAX_BYTES: usize = 16 * 1024;
/// Encoded schema this build accepts; version 2 added the required update policy, version 3 the
/// required release-notes location.
pub const SCHEMA_VERSION: u32 = 3;
/// Placeholder that a release-notes location replaces with the version it describes.
pub const VERSION_PLACEHOLDER: &str = "{version}";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Configuration {
    schema_version: u32,
    repository: [u8; 32],
    feed: Feed,
    update_policy: UpdatePolicy,
    release_notes: String,
}

#[derive(Deserialize)]
#[serde(tag = "transport", rename_all = "snake_case", deny_unknown_fields)]
enum Feed {
    Https {
        base: String,
        redirects: Vec<String>,
    },
    #[cfg(any(test, feature = "boundary-fixtures"))]
    Loopback {
        port: u16,
    },
}

/// How a build applies application updates. Compiled into each build's distribution beside its
/// repository and feed; never read from signed metadata, the environment or user settings, so it
/// cannot be switched remotely. Graduating from beta means releasing a build whose policy is
/// `Standard`.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum UpdatePolicy {
    /// At launch, check, download and apply an application update before the app opens, with
    /// the splash showing progress. Any failure starts the current version and files a report.
    BetaEager,
    /// Prepare updates silently in the background, tell the user one is ready, and apply it at
    /// the next launch or when the user chooses to restart.
    Standard,
}

/// Validated producing-build inputs. This binds a feed to existing installed trust; it cannot
/// initialize that trust, grant package authority, choose a channel, or claim an installed version.
/// The update policy is compiled in with the feed, so no remote input can switch it.
pub struct Distribution {
    repository: [u8; 32],
    policy: Policy,
    update_policy: UpdatePolicy,
    release_notes: ReleaseNotes,
}

/// Where a build's release notes are published: an https URL template containing
/// [VERSION_PLACEHOLDER] once. Release CI publishes each version's generated changelog section
/// at that address (for SugarSubstitute, the GitHub release substitute-v{version}), so the
/// "What's new" link of a version opens exactly the notes semantic-release wrote for it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleaseNotes {
    template: String,
}
impl ReleaseNotes {
    /// Validate a template at decode time, so a link for any valid version is a plain https URL.
    fn new(template: String) -> io::Result<Self> {
        let notes = Self { template };
        if notes.template.matches(VERSION_PLACEHOLDER).count() != 1 {
            return Err(invalid("Release notes location must contain one version placeholder"));
        }
        notes.url("0.0.0")?;
        Ok(notes)
    }

    /// The release notes of `version`, a plain `major.minor.patch` release version.
    /// # Errors
    /// Rejects any other version text, and a location that is not plain https with a host and
    /// without credentials or a query.
    pub fn url(&self, version: &str) -> io::Result<url::Url> {
        let mut parts = version.split('.');
        let plain = (0..3).all(|_| {
            parts.next().is_some_and(|part| {
                !part.is_empty()
                    && part.bytes().all(|byte| byte.is_ascii_digit())
                    && (part == "0" || !part.starts_with('0'))
            })
        }) && parts.next().is_none();
        if !plain {
            return Err(invalid("Release notes need a plain release version"));
        }
        let url = url::Url::parse(&self.template.replace(VERSION_PLACEHOLDER, version))
            .map_err(|cause| io::Error::new(io::ErrorKind::InvalidData, cause))?;
        if url.scheme() != "https"
            || url.host_str().is_none_or(str::is_empty)
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
        {
            return Err(invalid("Release notes location must be a plain https address"));
        }
        Ok(url)
    }
}
impl Distribution {
    /// Decode bounded bytes compiled into the authenticated producing application. Call on an
    /// admitted preparation job. Never populate these inputs from downloaded metadata, runtime
    /// environment variables, neighboring files or the embedded web interface.
    /// # Errors
    /// Rejects empty/oversized bytes, unknown schemas/fields, empty repository identities,
    /// invalid origins and a release-notes location that is not one plain https template.
    /// Ordinary builds accept only HTTPS; fixture HTTP is literal loopback only.
    pub fn decode(bytes: &[u8]) -> io::Result<Self> {
        if bytes.is_empty() || bytes.len() > MAX_BYTES {
            return Err(invalid("Update distribution exceeds its encoded bounds"));
        }
        let configuration: Configuration = serde_json::from_slice(bytes)
            .map_err(|cause| io::Error::new(io::ErrorKind::InvalidData, cause))?;
        if configuration.schema_version != SCHEMA_VERSION || configuration.repository == [0; 32] {
            return Err(invalid("Update distribution schema or repository is invalid"));
        }
        let policy = match configuration.feed {
            Feed::Https { base, redirects } => {
                Policy::https(&base, &redirects.iter().map(String::as_str).collect::<Vec<_>>())
            }
            #[cfg(any(test, feature = "boundary-fixtures"))]
            Feed::Loopback { port } => Policy::loopback(port),
        }
        .map_err(|cause| io::Error::new(io::ErrorKind::InvalidData, cause))?;
        Ok(Self {
            repository: configuration.repository,
            policy,
            update_policy: configuration.update_policy,
            release_notes: ReleaseNotes::new(configuration.release_notes)?,
        })
    }

    /// Where this build's release notes are published, for the "What's new" link.
    pub fn release_notes(&self) -> &ReleaseNotes {
        &self.release_notes
    }

    /// How this build applies application updates.
    pub fn update_policy(&self) -> UpdatePolicy {
        self.update_policy
    }

    /// Transfer the configured binding and origin policy to the native preparation owner.
    pub fn into_parts(self) -> ([u8; 32], Policy) {
        (self.repository, self.policy)
    }
}

/// Keep malformed build input observable without treating it as disabled updates.
fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
