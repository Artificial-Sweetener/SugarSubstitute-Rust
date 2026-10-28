// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode bounded panel messages through generated closed schema shapes.

use super::{
    MAX_PANEL_MESSAGE_BYTES, PanelError, PanelMessage, PanelPresentation, PanelRejection,
    PanelSession, wire,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct Discriminator {
    #[serde(rename = "type")]
    kind: String,
}

impl PanelMessage {
    /// Decode one closed version-three panel message without authorizing its sender.
    ///
    /// The transport must independently verify the current navigation, frame and session.
    ///
    /// # Errors
    /// Rejects oversized, malformed, duplicate-field, extended or schema-invalid messages.
    pub fn decode(bytes: &[u8]) -> Result<Self, PanelError> {
        if bytes.len() > MAX_PANEL_MESSAGE_BYTES {
            return Err(PanelError::TooLarge);
        }
        let discriminator: Discriminator =
            serde_json::from_slice(bytes).map_err(PanelError::Malformed)?;
        match discriminator.kind.as_str() {
            "ready" => {
                let value: wire::PanelReady = parse(bytes)?;
                if !value.valid() {
                    return Err(PanelError::Invalid);
                }
                Ok(Self::Ready(PanelSession::new(value.session)?))
            }
            "configure" => {
                let value: wire::PanelConfigure = parse(bytes)?;
                if !value.valid() {
                    return Err(PanelError::Invalid);
                }
                Ok(Self::Configure {
                    session: PanelSession::new(value.session)?,
                    presentation: PanelPresentation::from_wire(value.presentation)?,
                })
            }
            "applied" => {
                let value: wire::PanelApplied = parse(bytes)?;
                if !value.valid() {
                    return Err(PanelError::Invalid);
                }
                Ok(Self::Applied(PanelSession::new(value.session)?))
            }
            "ping" => {
                let value: wire::PanelPing = parse(bytes)?;
                if !value.valid() {
                    return Err(PanelError::Invalid);
                }
                Ok(Self::Ping(PanelSession::new(value.session)?))
            }
            "pong" => {
                let value: wire::PanelPong = parse(bytes)?;
                if !value.valid() {
                    return Err(PanelError::Invalid);
                }
                Ok(Self::Pong {
                    session: PanelSession::new(value.session)?,
                    connected: value.connected,
                })
            }
            "rejected" => {
                let value: wire::PanelRejected = parse(bytes)?;
                if !value.valid() {
                    return Err(PanelError::Invalid);
                }
                let reason = match value.reason.as_str() {
                    "unsupported-presentation" => PanelRejection::UnsupportedPresentation,
                    "presentation-failed" => PanelRejection::PresentationFailed,
                    _ => return Err(PanelError::Invalid),
                };
                Ok(Self::Rejected { session: PanelSession::new(value.session)?, reason })
            }
            _ => Err(PanelError::Invalid),
        }
    }
}

/// Preserve the actual decoder error without accepting unknown or duplicate wire fields.
fn parse<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> Result<T, PanelError> {
    serde_json::from_slice(bytes).map_err(PanelError::Malformed)
}
