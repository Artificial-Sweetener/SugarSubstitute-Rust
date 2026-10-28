// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Correlate the shared display-only canvas context without granting native authority.

mod wire;

use crate::panel::PanelSession;
use std::{error::Error, fmt};

/// Bound context requests before decoding or allocating their public fields.
pub const MAX_CANVAS_CONTEXT_BYTES: usize = 4096;

/// A public context read from one panel session; navigation authority is checked separately.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanvasContextRequest {
    session: PanelSession,
    request_id: u64,
}

/// Report whether the composing host can actually service output-surface operations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanvasDisplayCapability {
    /// The host cannot currently display output through the shared surface.
    Unavailable,
    /// The host has admitted a surface capable of displaying selected output.
    Available,
}

impl CanvasContextRequest {
    /// Decode the exact shared version-three context request.
    ///
    /// # Errors
    /// Rejects oversized, malformed, extended, duplicate or schema-invalid messages.
    pub fn decode(bytes: &[u8]) -> Result<Self, CanvasContextError> {
        if bytes.len() > MAX_CANVAS_CONTEXT_BYTES {
            return Err(CanvasContextError::TooLarge);
        }
        let request: wire::CanvasReadRequest =
            serde_json::from_slice(bytes).map_err(CanvasContextError::Malformed)?;
        if !request.valid() {
            return Err(CanvasContextError::Invalid);
        }
        Ok(Self {
            session: PanelSession::new(request.session).map_err(|_| CanvasContextError::Invalid)?,
            request_id: request.request_id,
        })
    }

    /// Borrow public correlation data; it is not an authentication credential.
    pub const fn session(&self) -> &PanelSession {
        &self.session
    }

    /// Return the original request identity within this panel session.
    pub const fn request_id(&self) -> u64 {
        self.request_id
    }

    /// Encode a display-only context tied to the exact original request.
    ///
    /// The caller must advertise availability only while it can fulfill surface operations.
    /// The response grants no layer, file, network, window or process privileges.
    ///
    /// # Errors
    /// Preserves schema or serialization failures and enforces the bounded reply size.
    pub fn reply(
        &self,
        capability: CanvasDisplayCapability,
    ) -> Result<Vec<u8>, CanvasContextError> {
        let reply = wire::CanvasNativeReadReply {
            protocol: "sugarcubes.canvas".into(),
            version: 3,
            session: self.session.as_str().into(),
            request_id: self.request_id,
            r#type: "context.reply".into(),
            context: wire::CanvasNativeContext {
                canvas: wire::CanvasNativeDescriptor {
                    id: "sugarsubstitute".into(),
                    label: "SugarSubstitute".into(),
                    capabilities: wire::CanvasNativeCapabilities {
                        display_output: capability == CanvasDisplayCapability::Available,
                        insert_image_layer: false,
                        position_image_layer: false,
                    },
                },
                document: (),
            },
        };
        if !reply.valid() {
            return Err(CanvasContextError::Invalid);
        }
        let bytes = serde_json::to_vec(&reply).map_err(CanvasContextError::Malformed)?;
        if bytes.len() > MAX_CANVAS_CONTEXT_BYTES {
            return Err(CanvasContextError::TooLarge);
        }
        Ok(bytes)
    }
}

/// Distinguish a bounded schema rejection from its actual JSON cause.
#[derive(Debug)]
pub enum CanvasContextError {
    /// The payload exceeds the fixed allocation bound.
    TooLarge,
    /// JSON decoding or encoding failed.
    Malformed(serde_json::Error),
    /// The message does not satisfy the shared schema.
    Invalid,
}

impl fmt::Display for CanvasContextError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::TooLarge => "Canvas context message exceeds its byte bound",
            Self::Malformed(_) => "Canvas context message is malformed",
            Self::Invalid => "Canvas context message violates its schema",
        })
    }
}

impl Error for CanvasContextError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Malformed(cause) => Some(cause),
            Self::TooLarge | Self::Invalid => None,
        }
    }
}

#[cfg(test)]
mod tests;
