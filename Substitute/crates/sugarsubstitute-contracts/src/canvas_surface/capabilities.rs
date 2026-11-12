// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Represent bounded runtime discovery without granting native surface authority.

use super::{CanvasSurfaceMessageError, MAX_SURFACE_MESSAGE_BYTES, wire};
use crate::panel::PanelSession;

/// Surface behavior advertised by its runtime owner; empty fields mean unsupported.
///
/// Callers publish only implemented runtime behavior. These values describe presentation
/// capabilities and never authorize navigation, native handles, or attachment operations.
/// Discovery schema one is decoded only to return an empty unsupported result with an
/// upgrade diagnostic. Hosts and SugarCubes upgrade together; canvas envelope version three
/// and presentation observation schema one retain their independent meanings.
/// Discovery schema three adds native backdrop regions; schema-two replies omit that field.
/// A request for a newer schema is answered in the newest schema this host knows.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanvasSurfaceCapabilities {
    /// Admitted selected-image encodings, bounded to three entries by the shared schema.
    pub image_formats: Vec<CanvasSurfaceImageFormat>,
    /// Supported viewport intents, bounded to six entries by the shared schema.
    pub viewport_commands: Vec<CanvasSurfaceViewportCommand>,
    /// Whether the surface publishes observable viewport state.
    pub viewport_state: bool,
    /// Encodings supported by preview presentation, bounded to two entries.
    pub preview_formats: Vec<CanvasSurfacePreviewFormat>,
    /// Whether the surface presents comparison content.
    pub comparison: bool,
    /// Whether the surface presents an image overview.
    pub overview: bool,
    /// Whether the host routes pointer and wheel gestures against page-published occlusions.
    /// Only such a host accepts geometry occlusions and the modal flag.
    pub input: bool,
    /// Whether schema-three clients can publish backdrop schema-one presentation regions.
    pub backdrop: bool,
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
    /// Move the comparison wipe divider; offered only with comparison.
    Wipe,
    /// Restore image-relative cameras on selection and on the shown image, reporting each
    /// viewport state's normalized center.
    Inspect,
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

/// Newest native surface discovery schema this host implements.
const NEWEST_DISCOVERY_SCHEMA: u64 = 3;

/// A schema-valid discovery request whose session still requires independent navigation checks.
#[derive(Clone, Debug, PartialEq)]
pub struct CanvasSurfaceDiscoveryRequest {
    session: PanelSession,
    request_id: u64,
    schema: u64,
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
            schema: value.schema,
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

    /// Return the requested feature schema; schema one permits only an unsupported reply.
    pub const fn schema(&self) -> u64 {
        self.schema
    }

    /// Encode implemented capabilities for this exact discovery request.
    ///
    /// Non-available outcomes require the empty default descriptor. Discovery reports
    /// runtime support and grants no native authority. Message length is measured in
    /// Unicode scalar values, and the complete reply retains the surface byte budget.
    /// Schema-one requests receive an empty unsupported descriptor and an upgrade diagnostic.
    /// A request for a schema newer than this host's is answered in the newest schema it knows,
    /// so the page keeps what that schema expresses and treats the rest as unsupported.
    /// # Errors
    /// Rejects nonempty failure descriptors, oversized lists or text, and JSON failures.
    pub fn reply(
        &self,
        status: CanvasSurfaceDiscoveryStatus,
        capabilities: &CanvasSurfaceCapabilities,
        message: &str,
    ) -> Result<Vec<u8>, CanvasSurfaceMessageError> {
        let unsupported = CanvasSurfaceCapabilities::default();
        let (status, capabilities, message) = if self.schema == 1 {
            (
                CanvasSurfaceDiscoveryStatus::Unsupported,
                &unsupported,
                "Native surface schema 2 is required. Update SugarCubes and its native host together.",
            )
        } else {
            (status, capabilities, message)
        };
        if (status != CanvasSurfaceDiscoveryStatus::Available
            && capabilities != &CanvasSurfaceCapabilities::default())
            || message.chars().count() > 512
        {
            return Err(CanvasSurfaceMessageError::Invalid);
        }
        let answered = self.schema.min(NEWEST_DISCOVERY_SCHEMA);
        let mut capabilities = capabilities.to_wire()?;
        capabilities.schema = answered;
        if answered < 3 {
            capabilities.backdrop = None;
        }
        let value = wire::CanvasSurfaceCapabilitiesReply {
            protocol: "sugarcubes.canvas".into(),
            version: 3,
            session: self.session.as_str().into(),
            request_id: self.request_id,
            r#type: "surface.capabilities.reply".into(),
            status: status.as_str().into(),
            capabilities,
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
            || self.viewport_commands.len() > 6
            || self.preview_formats.len() > 2
        {
            return Err(CanvasSurfaceMessageError::Invalid);
        }
        let value = wire::CanvasSurfaceCapabilities {
            schema: 2,
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
            input: self.input,
            backdrop: self.backdrop.then_some(wire::CanvasSurfaceBackdropSupport { schema: 1 }),
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
            Self::Wipe => "wipe",
            Self::Inspect => "inspect",
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
    pub(super) const fn as_str(self) -> &'static str {
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
