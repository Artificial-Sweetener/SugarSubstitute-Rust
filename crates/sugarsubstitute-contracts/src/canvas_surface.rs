// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate untrusted shared canvas geometry independently of native surface authority.

mod address;
mod capabilities;
mod file_reference;
mod message;
mod reply;
#[cfg(test)]
#[path = "canvas_surface/tests.rs"]
mod tests;
mod viewport;
#[path = "canvas_surface/wire.rs"]
mod wire;

pub use address::{CanvasSurfaceAddress, CanvasSurfaceAddressError, MAX_SURFACE_ADDRESS_BYTES};
pub use capabilities::{
    CanvasSurfaceCapabilities, CanvasSurfaceDiscoveryRequest, CanvasSurfaceDiscoveryStatus,
    CanvasSurfaceImageFormat, CanvasSurfacePreviewFormat, CanvasSurfaceViewportCommand,
};
pub use file_reference::{
    CanvasFileError, CanvasFileReference, CanvasFileStorage, MAX_FILE_REFERENCE_BYTES,
};
pub use message::{
    CanvasSurfaceMessage, CanvasSurfaceMessageError, CanvasSurfaceOperation,
    CanvasSurfaceReplyStatus, CanvasSurfaceRequest, MAX_SURFACE_MESSAGE_BYTES,
};
pub use viewport::{CanvasViewportCommand, CanvasViewportMode, CanvasViewportState};

use std::{error::Error, fmt};

/// Maximum serialized geometry size before JSON decoding, in UTF-8 bytes.
pub const MAX_GEOMETRY_BYTES: usize = 2048;

/// A schema-valid geometry snapshot; it grants no navigation or host authority.
#[derive(Clone, Debug, PartialEq)]
pub struct CanvasSurfaceGeometry(wire::CanvasSurfaceGeometry);

impl CanvasSurfaceGeometry {
    /// Adopt generated data only after validating the same shared schema.
    fn from_wire(value: wire::CanvasSurfaceGeometry) -> Option<Self> {
        value.valid().then_some(Self(value))
    }
    /// Decode the shared closed geometry shape, including all numeric bounds.
    ///
    /// Coordinates and sizes are CSS pixels in the current web client coordinate space.
    /// The caller must independently verify the producing navigation and attachment identity.
    ///
    /// # Errors
    /// Rejects oversized, malformed, extended, or out-of-range input without changing state.
    pub fn decode(bytes: &[u8]) -> Result<Self, GeometryError> {
        if bytes.len() > MAX_GEOMETRY_BYTES {
            return Err(GeometryError::TooLarge);
        }
        let value: wire::CanvasSurfaceGeometry =
            serde_json::from_slice(bytes).map_err(GeometryError::Malformed)?;
        Self::from_wire(value).ok_or(GeometryError::OutOfBounds)
    }

    /// Return the viewport's client-space CSS-pixel origin.
    pub const fn origin(&self) -> (f64, f64) {
        (self.0.origin.x, self.0.origin.y)
    }

    /// Return the viewport's width and height in CSS pixels.
    pub const fn viewport(&self) -> (f64, f64) {
        (self.0.viewport.width, self.0.viewport.height)
    }

    /// Return the reported client-space CSS-pixel clip as x, y, width and height.
    pub const fn clip(&self) -> (f64, f64, f64, f64) {
        (self.0.clip.x, self.0.clip.y, self.0.clip.width, self.0.clip.height)
    }

    /// Return the web-observed physical-pixels-per-CSS-pixel ratio.
    /// Native placement must also verify the actual window's current scale and bounds.
    pub const fn density(&self) -> f64 {
        self.0.physical_pixels_per_css_pixel
    }

    /// Return the page's reported visibility, independent of native visibility.
    pub const fn visible(&self) -> bool {
        self.0.visible
    }

    /// Return the page's reported focus, independent of native focus.
    pub const fn focused(&self) -> bool {
        self.0.focused
    }
}

/// Identify rejection while retaining an actual JSON decoder failure as its source.
#[derive(Debug)]
pub enum GeometryError {
    /// The encoded value exceeds the fixed admission bound.
    TooLarge,
    /// The JSON value does not match the closed schema's required fields and types.
    Malformed(serde_json::Error),
    /// A recognized field violates its schema constant or numeric bounds.
    OutOfBounds,
}

impl fmt::Display for GeometryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge => formatter.write_str("Canvas geometry exceeds its byte bound"),
            Self::Malformed(_) => formatter.write_str("Canvas geometry is malformed"),
            Self::OutOfBounds => formatter.write_str("Canvas geometry violates its bounds"),
        }
    }
}

impl Error for GeometryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Malformed(cause) => Some(cause),
            Self::TooLarge | Self::OutOfBounds => None,
        }
    }
}
