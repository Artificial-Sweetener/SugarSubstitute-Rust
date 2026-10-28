// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate the shared SugarCubes panel protocol independently of native transport authority.

mod decode;
mod encode;
mod presentation;
#[cfg(test)]
mod tests;
mod wire;

use sha2::{Digest, Sha256};
use std::{error::Error, fmt};

pub use presentation::{PanelPalette, PanelPresentation, PanelStyle};

/// Maximum encoded panel message admitted before JSON decoding, in UTF-8 bytes.
pub const MAX_PANEL_MESSAGE_BYTES: usize = 4096;

/// Correlate one public panel navigation without granting host privileges.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PanelSession(String);

impl PanelSession {
    /// Construct an identifier allowed by the shared version-three panel schema.
    ///
    /// # Errors
    /// Rejects non-ASCII, malformed, short or oversized identifiers.
    pub fn new(value: String) -> Result<Self, PanelError> {
        if wire::valid_panel_session(&value) {
            Ok(Self(value.into_boxed_str().into_string()))
        } else {
            Err(PanelError::Invalid)
        }
    }

    /// Derive a correlation label from the existing complete main-navigation identity.
    ///
    /// This deterministic value is public presentation data, not a secret or authorization token.
    ///
    /// # Errors
    /// Rejects a generated spelling if it ever diverges from the shared schema.
    pub fn for_main(identity: crate::startup::MainIdentity) -> Result<Self, PanelError> {
        let mut digest = Sha256::new();
        digest.update(b"sugarsubstitute.panel.v3\0");
        digest.update(identity.run.owner.namespace.bytes());
        digest.update(identity.run.owner.epoch.bytes());
        for value in [
            identity.run.instance.get(),
            identity.run.generation.get(),
            identity.launch.get(),
            identity.navigation.get(),
        ] {
            digest.update(value.to_le_bytes());
        }
        Self::new(format!("panel-{:x}", digest.finalize()))
    }

    /// Borrow the correlation identifier; it is not an authorization credential.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The panel client's schema-defined reasons for declining configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PanelRejection {
    /// The requested presentation is unsupported.
    UnsupportedPresentation,
    /// The presentation could not be applied.
    PresentationFailed,
}

/// One validated panel message. Navigation and operation authority remain host-owned.
#[derive(Clone, Debug, PartialEq)]
pub enum PanelMessage {
    /// A page announces that it can receive its host configuration.
    Ready(PanelSession),
    /// A host supplies the presentation for this page.
    Configure {
        /// The current panel navigation.
        session: PanelSession,
        /// The host's validated presentation values.
        presentation: PanelPresentation,
    },
    /// The page applied its configuration.
    Applied(PanelSession),
    /// The host requests a connectivity observation.
    Ping(PanelSession),
    /// The page reports its current Comfy connection state.
    Pong {
        /// The current panel navigation.
        session: PanelSession,
        /// Whether the page reports an active Comfy connection.
        connected: bool,
    },
    /// The page declined its configuration.
    Rejected {
        /// The current panel navigation.
        session: PanelSession,
        /// The closed reason defined by the shared contract.
        reason: PanelRejection,
    },
}

impl PanelMessage {
    /// Borrow the identifier addressed by this message.
    pub fn session(&self) -> &PanelSession {
        match self {
            Self::Ready(session) | Self::Applied(session) | Self::Ping(session) => session,
            Self::Configure { session, .. }
            | Self::Pong { session, .. }
            | Self::Rejected { session, .. } => session,
        }
    }
}

/// Identify a bounded wire failure while retaining actual JSON errors as causes.
#[derive(Debug)]
pub enum PanelError {
    /// The encoded message exceeded its fixed admission bound.
    TooLarge,
    /// JSON does not match a closed message shape.
    Malformed(serde_json::Error),
    /// A field or message kind violates the shared schema.
    Invalid,
}

impl fmt::Display for PanelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge => formatter.write_str("Panel message exceeds its byte bound"),
            Self::Malformed(_) => formatter.write_str("Panel message is malformed"),
            Self::Invalid => formatter.write_str("Panel message violates its schema"),
        }
    }
}

impl Error for PanelError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Malformed(cause) => Some(cause),
            Self::TooLarge | Self::Invalid => None,
        }
    }
}
