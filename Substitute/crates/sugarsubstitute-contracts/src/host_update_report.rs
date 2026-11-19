// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Tell the embedded SugarCubes page whether the host has its own report of an update to show,
//! and take its request to show it (`sugarcubes.update-report` v1).
//!
//! The page sends `watch` once the host description lists the protocol; the host answers every
//! watch with a state and sends a new one, with a higher revision, whenever it changes. The
//! page asks for the report with `open`, naming the revision of the state it saw. Decoding
//! validates shape and bounds only; whether an open is acted on is the host's decision.

use crate::panel::{PanelError, PanelSession};
use std::{error::Error, fmt};

mod wire;

#[cfg(test)]
mod tests;

/// Maximum encoded update report message admitted or produced, in UTF-8 bytes.
pub const MAX_UPDATE_REPORT_MESSAGE_BYTES: usize = 1024;
/// Largest revision the page's number type carries exactly.
pub const MAX_UPDATE_REPORT_REVISION: u64 = 9_007_199_254_740_991;
const PROTOCOL: &str = "sugarcubes.update-report";
const VERSION: u64 = 1;

/// One host-direction update report message.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpdateReportHostMessage {
    /// Whether the host has a report as of `revision`, which grows with every change.
    State {
        /// Zero until the first change; at most [`MAX_UPDATE_REPORT_REVISION`].
        revision: u64,
        /// Whether the host has a report to show now.
        available: bool,
    },
}

impl UpdateReportHostMessage {
    /// Encode this message addressed to `session`.
    ///
    /// # Errors
    /// Rejects a revision beyond the bound, generated-contract drift, encoding failure or an
    /// exceeded byte bound.
    pub fn encode(&self, session: &PanelSession) -> Result<Vec<u8>, UpdateReportError> {
        let value = wire::encode(self, session.as_str());
        if !value.valid() {
            return Err(UpdateReportError::Invalid);
        }
        let encoded = serde_json::to_vec(&value).map_err(UpdateReportError::Malformed)?;
        if encoded.len() > MAX_UPDATE_REPORT_MESSAGE_BYTES {
            return Err(UpdateReportError::TooLarge);
        }
        Ok(encoded)
    }
}

/// One page-direction update report message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UpdateReportClientMessage {
    /// The page can offer the report and asks whether there is one.
    Watch {
        /// The current panel navigation.
        session: PanelSession,
    },
    /// The user asked to see the report.
    Open {
        /// The current panel navigation.
        session: PanelSession,
        /// The revision of the state the page showed.
        revision: u64,
    },
}

impl UpdateReportClientMessage {
    /// Borrow the identifier addressed by this message.
    pub fn session(&self) -> &PanelSession {
        match self {
            Self::Watch { session } | Self::Open { session, .. } => session,
        }
    }

    /// Decode one page-direction message without authorizing its sender.
    ///
    /// # Errors
    /// Rejects oversized, malformed, extended or out-of-bounds messages and foreign protocols.
    pub fn decode(bytes: &[u8]) -> Result<Self, UpdateReportError> {
        if bytes.len() > MAX_UPDATE_REPORT_MESSAGE_BYTES {
            return Err(UpdateReportError::TooLarge);
        }
        let message: wire::UpdateReportClientMessage =
            serde_json::from_slice(bytes).map_err(UpdateReportError::Malformed)?;
        if !message.valid() {
            return Err(UpdateReportError::Invalid);
        }
        wire::decode(message)
    }
}

/// Identify a bounded wire failure while retaining actual JSON errors as causes.
#[derive(Debug)]
pub enum UpdateReportError {
    /// The message exceeded its fixed byte bound.
    TooLarge,
    /// JSON does not match a closed message shape.
    Malformed(serde_json::Error),
    /// A field violates the shared schema.
    Invalid,
    /// The addressed panel session is invalid.
    Session(PanelError),
}

impl fmt::Display for UpdateReportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge => formatter.write_str("Update report message exceeds its byte bound"),
            Self::Malformed(_) => formatter.write_str("Update report message is malformed"),
            Self::Invalid => formatter.write_str("Update report message violates its schema"),
            Self::Session(_) => {
                formatter.write_str("Update report message names an invalid session")
            }
        }
    }
}

impl Error for UpdateReportError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Malformed(cause) => Some(cause),
            Self::Session(cause) => Some(cause),
            Self::TooLarge | Self::Invalid => None,
        }
    }
}
