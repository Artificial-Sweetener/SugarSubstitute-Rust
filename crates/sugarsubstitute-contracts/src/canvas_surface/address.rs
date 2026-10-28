// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode a shared attachment address without treating it as authority.

use super::wire;
use std::{error::Error, fmt};

/// Maximum serialized attachment address before JSON decoding, in UTF-8 bytes.
pub const MAX_SURFACE_ADDRESS_BYTES: usize = 8192;

/// Schema-valid correlation data; the actual navigation and attachment still require verification.
#[derive(Clone, Debug, PartialEq)]
pub struct CanvasSurfaceAddress(wire::CanvasSurfaceAddress);

impl CanvasSurfaceAddress {
    /// Retain an already-decoded address only after shared-schema validation.
    pub(super) fn from_wire(value: wire::CanvasSurfaceAddress) -> Option<Self> {
        value.valid().then_some(Self(value))
    }

    /// Borrow the validated generated value when encoding a matching response.
    pub(super) fn wire(&self) -> &wire::CanvasSurfaceAddress {
        &self.0
    }
    /// Decode the complete closed address with bounded, well-formed identity components.
    ///
    /// A matching address does not authorize filesystem access, network access, or a native handle.
    /// # Errors
    /// Rejects oversized, malformed, extended, or schema-invalid input.
    pub fn decode(bytes: &[u8]) -> Result<Self, CanvasSurfaceAddressError> {
        if bytes.len() > MAX_SURFACE_ADDRESS_BYTES {
            return Err(CanvasSurfaceAddressError::TooLarge);
        }
        let value: wire::CanvasSurfaceAddress =
            serde_json::from_slice(bytes).map_err(CanvasSurfaceAddressError::Malformed)?;
        Self::from_wire(value).ok_or(CanvasSurfaceAddressError::OutOfBounds)
    }

    /// Borrow the SugarCubes canvas-session identity.
    pub fn session_id(&self) -> &str {
        &self.0.session_id
    }

    /// Borrow the SugarCubes view identity.
    pub fn view_id(&self) -> &str {
        &self.0.view_id
    }

    /// Borrow the presentation window identity.
    pub fn window_id(&self) -> &str {
        &self.0.window_id
    }

    /// Borrow the single-use surface attachment identity.
    pub fn attachment_id(&self) -> &str {
        &self.0.attachment_id
    }

    /// Return the positive generation within JavaScript's exact integer range.
    pub const fn generation(&self) -> u64 {
        self.0.generation
    }
}

/// Preserve decoding failures while distinguishing schema and admission bounds.
#[derive(Debug)]
pub enum CanvasSurfaceAddressError {
    /// The encoded address exceeds its fixed admission bound.
    TooLarge,
    /// The input is not the required closed JSON shape.
    Malformed(serde_json::Error),
    /// A field violates its identity grammar or generation bound.
    OutOfBounds,
}

impl fmt::Display for CanvasSurfaceAddressError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge => formatter.write_str("Canvas surface address exceeds its byte bound"),
            Self::Malformed(_) => formatter.write_str("Canvas surface address is malformed"),
            Self::OutOfBounds => formatter.write_str("Canvas surface address violates its bounds"),
        }
    }
}

impl Error for CanvasSurfaceAddressError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Malformed(cause) => Some(cause),
            Self::TooLarge | Self::OutOfBounds => None,
        }
    }
}
