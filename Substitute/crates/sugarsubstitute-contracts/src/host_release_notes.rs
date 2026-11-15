// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exchange the host's own release notes with the embedded SugarCubes page
//! (`sugarcubes.release-notes` v1).
//!
//! The page sends `watch` once the host description lists the protocol; the host answers with
//! its product name and running version. The host sends `open` when the user asks the host to
//! show what is new, and the page shows its release notes view. The page asks for the notes of
//! every version after the one it last showed; the host answers each request by name with the
//! published notes, newest first, or with why it has none. Decoding validates shape and bounds
//! only; it grants no host authority.

use crate::panel::{PanelError, PanelSession};
use std::{error::Error, fmt};

mod wire;

#[cfg(test)]
mod tests;

/// Maximum encoded release notes message admitted or produced, in UTF-8 bytes.
pub const MAX_RELEASE_NOTES_MESSAGE_BYTES: usize = 1024 * 1024;
/// Most versions one answer carries.
pub const MAX_RELEASES: usize = 64;
/// Longest notes of one version, in UTF-16 code units, as the page measures text.
pub const MAX_NOTES_UNITS: usize = 262_144;
/// Longest product name, in UTF-16 code units.
pub const MAX_PRODUCT_UNITS: usize = 64;
const PROTOCOL: &str = "sugarcubes.release-notes";
const VERSION: u64 = 1;

/// A lowercase release token, 1 to 64 of `a-z`, `0-9`, `.`, `-`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleaseNotesVersion(String);

impl ReleaseNotesVersion {
    /// Accept a release label that is already a schema token.
    ///
    /// # Errors
    /// Rejects an empty, oversized or non-token label.
    pub fn new(value: &str) -> Result<Self, ReleaseNotesError> {
        if token(value, 64) { Ok(Self(value.to_owned())) } else { Err(ReleaseNotesError::Invalid) }
    }

    /// Borrow the token.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The page's own name for one request, 1 to 48 of `a-z`, `0-9`, `.`, `-`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleaseNotesRequestName(String);

impl ReleaseNotesRequestName {
    /// Accept a request name that is a schema token.
    ///
    /// # Errors
    /// Rejects an empty, oversized or non-token name.
    pub fn new(value: &str) -> Result<Self, ReleaseNotesError> {
        if token(value, 48) { Ok(Self(value.to_owned())) } else { Err(ReleaseNotesError::Invalid) }
    }

    /// Borrow the name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The published notes of one version: the Markdown its changelog holds, heading included.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleaseNotesRelease {
    version: ReleaseNotesVersion,
    notes: String,
}

impl ReleaseNotesRelease {
    /// Retain `notes` of at most [`MAX_NOTES_UNITS`] UTF-16 code units for `version`.
    ///
    /// # Errors
    /// Rejects longer notes.
    pub fn new(version: ReleaseNotesVersion, notes: String) -> Result<Self, ReleaseNotesError> {
        if notes.encode_utf16().count() > MAX_NOTES_UNITS {
            return Err(ReleaseNotesError::Invalid);
        }
        Ok(Self { version, notes })
    }

    /// The version these notes describe.
    pub fn version(&self) -> &ReleaseNotesVersion {
        &self.version
    }

    /// The Markdown text.
    pub fn notes(&self) -> &str {
        &self.notes
    }
}

/// Why the host has no notes to show.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReleaseNotesFailure {
    /// The notes could not be read now; a retry may succeed.
    Unavailable,
    /// The release repository has no notes for the running version.
    NotPublished,
}

/// One host-direction release notes message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReleaseNotesHostMessage {
    /// The host's product and running version; `revision` increases with every change within a
    /// panel session.
    State {
        /// At least 1; the page ignores a state that is not newer than the last it applied.
        revision: u64,
        /// The host's display name, at most [`MAX_PRODUCT_UNITS`] UTF-16 code units.
        product: String,
        /// The running version.
        current_version: ReleaseNotesVersion,
    },
    /// The user asked the host to show what is new.
    Open,
    /// The published notes the named request asked for, newest version first.
    Notes {
        /// The page's request name.
        request: ReleaseNotesRequestName,
        /// One to [`MAX_RELEASES`] versions.
        releases: Vec<ReleaseNotesRelease>,
        /// Older versions in the range left out to keep the message within its bound.
        omitted: u64,
    },
    /// The host has no notes for the named request.
    Failed {
        /// The page's request name.
        request: ReleaseNotesRequestName,
        /// Why.
        reason: ReleaseNotesFailure,
    },
}

impl ReleaseNotesHostMessage {
    /// Encode this message addressed to `session`.
    ///
    /// # Errors
    /// Rejects a value outside the shared schema bounds, generated-contract drift, encoding
    /// failure or an exceeded byte bound.
    pub fn encode(&self, session: &PanelSession) -> Result<Vec<u8>, ReleaseNotesError> {
        let value = wire::encode(self, session.as_str());
        if !value.valid() {
            return Err(ReleaseNotesError::Invalid);
        }
        let encoded = serde_json::to_vec(&value).map_err(ReleaseNotesError::Malformed)?;
        if encoded.len() > MAX_RELEASE_NOTES_MESSAGE_BYTES {
            return Err(ReleaseNotesError::TooLarge);
        }
        Ok(encoded)
    }
}

/// One page-direction release notes message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReleaseNotesClientMessage {
    /// The page starts following the host's product and version.
    Watch(PanelSession),
    /// The page asks for the notes of every version after `since`, through the running one.
    Load {
        /// The current panel navigation.
        session: PanelSession,
        /// The page's request name.
        request: ReleaseNotesRequestName,
        /// The version the page last showed; one not older than the running version asks for
        /// the running version alone.
        since: ReleaseNotesVersion,
    },
}

impl ReleaseNotesClientMessage {
    /// Borrow the identifier addressed by this message.
    pub fn session(&self) -> &PanelSession {
        match self {
            Self::Watch(session) | Self::Load { session, .. } => session,
        }
    }

    /// Decode one page-direction message without authorizing its sender.
    ///
    /// # Errors
    /// Rejects oversized, malformed, extended or out-of-bounds messages and foreign protocols.
    pub fn decode(bytes: &[u8]) -> Result<Self, ReleaseNotesError> {
        if bytes.len() > MAX_RELEASE_NOTES_MESSAGE_BYTES {
            return Err(ReleaseNotesError::TooLarge);
        }
        let message: wire::ReleaseNotesClientMessage =
            serde_json::from_slice(bytes).map_err(ReleaseNotesError::Malformed)?;
        if !message.valid() {
            return Err(ReleaseNotesError::Invalid);
        }
        wire::decode(message)
    }
}

/// Identify a bounded wire failure while retaining actual JSON errors as causes.
#[derive(Debug)]
pub enum ReleaseNotesError {
    /// The message exceeded its fixed byte bound.
    TooLarge,
    /// JSON does not match a closed message shape.
    Malformed(serde_json::Error),
    /// A field violates the shared schema.
    Invalid,
    /// The addressed panel session is invalid.
    Session(PanelError),
}

impl fmt::Display for ReleaseNotesError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge => formatter.write_str("Release notes message exceeds its byte bound"),
            Self::Malformed(_) => formatter.write_str("Release notes message is malformed"),
            Self::Invalid => formatter.write_str("Release notes message violates its schema"),
            Self::Session(_) => {
                formatter.write_str("Release notes message names an invalid session")
            }
        }
    }
}

impl Error for ReleaseNotesError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Malformed(cause) => Some(cause),
            Self::Session(cause) => Some(cause),
            Self::TooLarge | Self::Invalid => None,
        }
    }
}

/// Whether `value` is a lowercase schema token of at most `maximum` bytes.
fn token(value: &str, maximum: usize) -> bool {
    (1..=maximum).contains(&value.len())
        && value.bytes().all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'.' | b'-'))
}
