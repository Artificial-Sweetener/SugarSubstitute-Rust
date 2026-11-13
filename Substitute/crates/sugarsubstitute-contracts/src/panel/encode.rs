// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Encode panel messages only after the generated schema validates the complete value.

use super::{MAX_PANEL_MESSAGE_BYTES, PanelError, PanelMessage, PanelRejection, wire};
use serde::Serialize;

const PROTOCOL: &str = "sugarcubes.panel";
const VERSION: u64 = 3;

impl PanelMessage {
    /// Encode one schema-valid panel message, without selecting a destination.
    ///
    /// # Errors
    /// Rejects generated-contract drift, encoding failure or an exceeded byte bound.
    pub fn encode(&self) -> Result<Vec<u8>, PanelError> {
        match self {
            Self::Ready(session) => {
                let value = wire::PanelReady {
                    protocol: PROTOCOL.into(),
                    version: VERSION,
                    session: session.as_str().into(),
                    r#type: "ready".into(),
                };
                encode(&value, value.valid())
            }
            Self::Configure { session, presentation } => {
                let value = wire::PanelConfigure {
                    protocol: PROTOCOL.into(),
                    version: VERSION,
                    session: session.as_str().into(),
                    r#type: "configure".into(),
                    presentation: presentation.wire(),
                };
                encode(&value, value.valid())
            }
            Self::Applied(session) => {
                let value = wire::PanelApplied {
                    protocol: PROTOCOL.into(),
                    version: VERSION,
                    session: session.as_str().into(),
                    r#type: "applied".into(),
                };
                encode(&value, value.valid())
            }
            Self::Ping(session) => {
                let value = wire::PanelPing {
                    protocol: PROTOCOL.into(),
                    version: VERSION,
                    session: session.as_str().into(),
                    r#type: "ping".into(),
                };
                encode(&value, value.valid())
            }
            Self::Pong { session, connected } => {
                let value = wire::PanelPong {
                    protocol: PROTOCOL.into(),
                    version: VERSION,
                    session: session.as_str().into(),
                    r#type: "pong".into(),
                    connected: *connected,
                };
                encode(&value, value.valid())
            }
            Self::Rejected { session, reason } => {
                let value = wire::PanelRejected {
                    protocol: PROTOCOL.into(),
                    version: VERSION,
                    session: session.as_str().into(),
                    r#type: "rejected".into(),
                    reason: match reason {
                        PanelRejection::UnsupportedPresentation => "unsupported-presentation",
                        PanelRejection::PresentationFailed => "presentation-failed",
                    }
                    .into(),
                };
                encode(&value, value.valid())
            }
        }
    }
}

/// Serialize only a value already accepted by the generated schema.
fn encode(value: &impl Serialize, valid: bool) -> Result<Vec<u8>, PanelError> {
    if !valid {
        return Err(PanelError::Invalid);
    }
    let encoded = serde_json::to_vec(value).map_err(PanelError::Malformed)?;
    if encoded.len() > MAX_PANEL_MESSAGE_BYTES {
        return Err(PanelError::TooLarge);
    }
    Ok(encoded)
}
