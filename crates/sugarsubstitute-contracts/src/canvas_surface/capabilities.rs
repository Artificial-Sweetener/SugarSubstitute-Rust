// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Represent bounded runtime discovery without granting native surface authority.

use super::{CanvasSurfaceMessageError, MAX_SURFACE_MESSAGE_BYTES, wire};
use crate::panel::PanelSession;

/// Surface behavior advertised by its runtime owner; empty fields mean unsupported behavior.
///
/// Callers publish only implemented runtime behavior. These values describe presentation
/// capabilities and never authorize navigation, native handles, or attachment operations.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanvasSurfaceCapabilities {
    /// Admitted selected-image encodings, bounded to three entries by the shared schema.
    pub image_formats: Vec<CanvasSurfaceImageFormat>,
    /// Supported viewport intents, bounded to four entries by the shared schema.
    pub viewport_commands: Vec<CanvasSurfaceViewportCommand>,
    /// Whether the surface publishes observable viewport state.
    pub viewport_state: bool,
    /// Encodings supported by preview presentation, bounded to two entries.
    pub preview_formats: Vec<CanvasSurfacePreviewFormat>,
    /// Whether the surface presents comparison content.
    pub comparison: bool,
    /// Whether the surface presents an image overview.
    pub overview: bool,
}

/// Selected-image encoding supported by a surface runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanvasSurfaceImageFormat {
    /// Portable Network Graphics.
    Png,
    /// Joint Photographic Experts Group image.
    Jpeg,
    /// WebP image.
    WebP,
}

/// Viewport operation supported by a surface runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanvasSurfaceViewportCommand {
    /// Fit the image to the available viewport.
    Fit,
    /// Show native image scale.
    Native,
    /// Change the viewport magnification.
    Zoom,
    /// Move the viewport over its image.
    Pan,
}

/// Preview encoding supported independently of selected-image presentation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanvasSurfacePreviewFormat {
    /// Portable Network Graphics.
    Png,
    /// Joint Photographic Experts Group image.
    Jpeg,
}

/// Correlated discovery outcome, independent of an attachment's lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanvasSurfaceDiscoveryStatus {
    /// The runtime supplied its implemented capabilities.
    Available,
    /// The host does not implement native surface discovery.
    Unsupported,
    /// The host cannot currently provide its surface runtime.
    Unavailable,
    /// Discovery failed.
    Error,
}

/// A schema-valid discovery request whose session still requires independent navigation checks.
#[derive(Clone, Debug, PartialEq)]
pub struct CanvasSurfaceDiscoveryRequest {
    session: PanelSession,
    request_id: u64,
}

impl CanvasSurfaceDiscoveryRequest {
    /// Adopt exact generated discovery data after validating version, schema and bounds.
    pub(super) fn from_wire(
        value: wire::CanvasSurfaceCapabilitiesRequest,
    ) -> Result<Self, CanvasSurfaceMessageError> {
        if !value.valid() {
            return Err(CanvasSurfaceMessageError::Invalid);
        }
        Ok(Self {
            session: PanelSession::new(value.session)
                .map_err(|_| CanvasSurfaceMessageError::Invalid)?,
            request_id: value.request_id,
        })
    }

    /// Borrow the public session label for independent current-navigation validation.
    pub fn session(&self) -> &PanelSession {
        &self.session
    }

    /// Return this request's identity within its panel session.
    pub const fn request_id(&self) -> u64 {
        self.request_id
    }

    /// Encode implemented capabilities for this exact discovery request.
    ///
    /// Non-available outcomes require the empty default descriptor. Discovery reports
    /// runtime support and grants no native authority. Message length is measured in
    /// Unicode scalar values, and the complete reply retains the surface byte budget.
    /// # Errors
    /// Rejects nonempty failure descriptors, oversized lists or text, and JSON failures.
    pub fn reply(
        &self,
        status: CanvasSurfaceDiscoveryStatus,
        capabilities: &CanvasSurfaceCapabilities,
        message: &str,
    ) -> Result<Vec<u8>, CanvasSurfaceMessageError> {
        if (status != CanvasSurfaceDiscoveryStatus::Available
            && capabilities != &CanvasSurfaceCapabilities::default())
            || message.chars().count() > 512
        {
            return Err(CanvasSurfaceMessageError::Invalid);
        }
        let value = wire::CanvasSurfaceCapabilitiesReply {
            protocol: "sugarcubes.canvas".into(),
            version: 3,
            session: self.session.as_str().into(),
            request_id: self.request_id,
            r#type: "surface.capabilities.reply".into(),
            status: status.as_str().into(),
            capabilities: capabilities.to_wire()?,
            message: message.into(),
        };
        if !value.valid() {
            return Err(CanvasSurfaceMessageError::Invalid);
        }
        let bytes = serde_json::to_vec(&value).map_err(CanvasSurfaceMessageError::Malformed)?;
        if bytes.len() > MAX_SURFACE_MESSAGE_BYTES {
            return Err(CanvasSurfaceMessageError::TooLarge);
        }
        Ok(bytes)
    }
}

impl CanvasSurfaceCapabilities {
    /// Bound collection allocation before translating the public descriptor to schema data.
    fn to_wire(&self) -> Result<wire::CanvasSurfaceCapabilities, CanvasSurfaceMessageError> {
        if self.image_formats.len() > 3
            || self.viewport_commands.len() > 4
            || self.preview_formats.len() > 2
        {
            return Err(CanvasSurfaceMessageError::Invalid);
        }
        let value = wire::CanvasSurfaceCapabilities {
            schema: 1,
            image_formats: self.image_formats.iter().map(|format| format.as_str().into()).collect(),
            viewport_commands: self
                .viewport_commands
                .iter()
                .map(|command| command.as_str().into())
                .collect(),
            viewport_state: self.viewport_state,
            preview_formats: self
                .preview_formats
                .iter()
                .map(|format| format.as_str().into())
                .collect(),
            comparison: self.comparison,
            overview: self.overview,
        };
        if !value.valid() {
            return Err(CanvasSurfaceMessageError::Invalid);
        }
        Ok(value)
    }
}

impl CanvasSurfaceImageFormat {
    /// Return the shared schema's selected-image encoding token.
    const fn as_str(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpeg",
            Self::WebP => "webp",
        }
    }
}

impl CanvasSurfaceViewportCommand {
    /// Return the shared schema's viewport intent token.
    const fn as_str(self) -> &'static str {
        match self {
            Self::Fit => "fit",
            Self::Native => "native",
            Self::Zoom => "zoom",
            Self::Pan => "pan",
        }
    }
}

impl CanvasSurfacePreviewFormat {
    /// Return the shared schema's preview encoding token.
    const fn as_str(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpeg",
        }
    }
}

impl CanvasSurfaceDiscoveryStatus {
    /// Return the shared schema's discovery outcome token.
    const fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Unsupported => "unsupported",
            Self::Unavailable => "unavailable",
            Self::Error => "error",
        }
    }
}

#[cfg(test)]
mod tests;
