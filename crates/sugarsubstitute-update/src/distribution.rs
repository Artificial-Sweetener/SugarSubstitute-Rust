// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Producing-build repository binding and transport policy, independent of signed remote offers.
use crate::http::Policy;
use serde::Deserialize;
use std::io;

/// Maximum encoded producing-build configuration, in bytes.
pub const MAX_BYTES: usize = 16 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Configuration {
    schema_version: u32,
    repository: [u8; 32],
    feed: Feed,
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

/// Validated producing-build inputs. This binds a feed to existing installed trust; it cannot
/// initialize that trust, grant package authority, choose a channel, or claim an installed version.
pub struct Distribution {
    repository: [u8; 32],
    policy: Policy,
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
        if configuration.schema_version != 1 || configuration.repository == [0; 32] {
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
        Ok(Self { repository: configuration.repository, policy })
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
