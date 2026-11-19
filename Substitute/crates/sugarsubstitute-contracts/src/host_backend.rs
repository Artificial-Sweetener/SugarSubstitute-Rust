// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Tell the embedded SugarCubes page whether the ComfyUI it talks to is running, and take its
//! request to start it again (`sugarcubes.backend` v1).
//!
//! The page sends `watch` once the host description lists the protocol; the host answers every
//! watch with a state and sends a new one, with a higher revision, whenever it changes. The
//! page asks for a restart with `retry`, naming the revision of the retry-ready state it saw.
//! Decoding validates shape and bounds only; whether a retry is acted on is the host's decision.

use crate::panel::{PanelError, PanelSession};
use std::{error::Error, fmt};

mod wire;

#[cfg(test)]
mod tests;

/// Maximum encoded backend message admitted or produced, in UTF-8 bytes.
pub const MAX_BACKEND_MESSAGE_BYTES: usize = 1024;
/// Largest revision the page's number type carries exactly.
pub const MAX_BACKEND_REVISION: u64 = 9_007_199_254_740_991;
const PROTOCOL: &str = "sugarcubes.backend";
const VERSION: u64 = 1;

/// What the host knows of the ComfyUI the page talks to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackendStatus {
    /// ComfyUI is up as far as the host knows.
    Running,
    /// It ended and the host is still settling what it left; it cannot be restarted yet.
    Stopped,
    /// The host can start it again when asked.
    RetryReady,
    /// A restart is under way.
    Retrying,
}

/// One host-direction backend message.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackendHostMessage {
    /// The backend's state as of `revision`, which grows with every change.
    State {
        /// Zero until the first change; at most [`MAX_BACKEND_REVISION`].
        revision: u64,
        /// What the host knows.
        status: BackendStatus,
    },
}

impl BackendHostMessage {
    /// Encode this message addressed to `session`.
    ///
    /// # Errors
    /// Rejects a revision beyond the bound, generated-contract drift, encoding failure or an
    /// exceeded byte bound.
    pub fn encode(&self, session: &PanelSession) -> Result<Vec<u8>, BackendError> {
        let value = wire::encode(self, session.as_str());
        if !value.valid() {
            return Err(BackendError::Invalid);
        }
        let encoded = serde_json::to_vec(&value).map_err(BackendError::Malformed)?;
        if encoded.len() > MAX_BACKEND_MESSAGE_BYTES {
            return Err(BackendError::TooLarge);
        }
        Ok(encoded)
    }
}

/// One page-direction backend message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BackendClientMessage {
    /// The page can show the backend's state and asks for it.
    Watch {
        /// The current panel navigation.
        session: PanelSession,
    },
    /// The user asked for the backend to be started again.
    Retry {
        /// The current panel navigation.
        session: PanelSession,
        /// The revision of the retry-ready state the page showed.
        revision: u64,
    },
}

impl BackendClientMessage {
    /// Borrow the identifier addressed by this message.
    pub fn session(&self) -> &PanelSession {
        match self {
            Self::Watch { session } | Self::Retry { session, .. } => session,
        }
    }

    /// Decode one page-direction message without authorizing its sender.
    ///
    /// # Errors
    /// Rejects oversized, malformed, extended or out-of-bounds messages and foreign protocols.
    pub fn decode(bytes: &[u8]) -> Result<Self, BackendError> {
        if bytes.len() > MAX_BACKEND_MESSAGE_BYTES {
            return Err(BackendError::TooLarge);
        }
        let message: wire::BackendClientMessage =
            serde_json::from_slice(bytes).map_err(BackendError::Malformed)?;
        if !message.valid() {
            return Err(BackendError::Invalid);
        }
        wire::decode(message)
    }
}

/// Identify a bounded wire failure while retaining actual JSON errors as causes.
#[derive(Debug)]
pub enum BackendError {
    /// The message exceeded its fixed byte bound.
    TooLarge,
    /// JSON does not match a closed message shape.
    Malformed(serde_json::Error),
    /// A field violates the shared schema.
    Invalid,
    /// The addressed panel session is invalid.
    Session(PanelError),
}

impl fmt::Display for BackendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge => formatter.write_str("Backend message exceeds its byte bound"),
            Self::Malformed(_) => formatter.write_str("Backend message is malformed"),
            Self::Invalid => formatter.write_str("Backend message violates its schema"),
            Self::Session(_) => formatter.write_str("Backend message names an invalid session"),
        }
    }
}

impl Error for BackendError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Malformed(cause) => Some(cause),
            Self::Session(cause) => Some(cause),
            Self::TooLarge | Self::Invalid => None,
        }
    }
}
