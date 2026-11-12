// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Producing-build repository binding, transport policy and update policy, independent of signed
//! remote offers.
use crate::http::Policy;
use serde::Deserialize;
use std::io;

/// Maximum encoded producing-build configuration, in bytes.
pub const MAX_BYTES: usize = 16 * 1024;
/// Encoded schema this build accepts; version 2 added the required update policy.
pub const SCHEMA_VERSION: u32 = 2;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Configuration {
    schema_version: u32,
    repository: [u8; 32],
    feed: Feed,
    update_policy: UpdatePolicy,
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
}
impl Distribution {
    /// Decode bounded bytes compiled into the authenticated producing application. Call on an
    /// admitted preparation job. Never populate these inputs from downloaded metadata, runtime
    /// environment variables, neighboring files or the embedded web interface.
    /// # Errors
    /// Rejects empty/oversized bytes, unknown schemas/fields, empty repository identities and
    /// invalid origins. Ordinary builds accept only HTTPS; fixture HTTP is literal loopback only.
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
        })
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
