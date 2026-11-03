// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Answer `context.request` with the canvas descriptor, capabilities and active document.

use super::{
    CanvasDocument, CanvasNativeEnvelope, CanvasNativeError, CanvasNativeReplyPayload,
    MAX_CANVAS_CONTEXT_REPLY_BYTES, MAX_CANVAS_CONTEXT_REQUEST_BYTES, ReplyEnvelope, admit, parse,
    wire,
};

/// A context read from one panel session; navigation authority is checked separately.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanvasContextRequest {
    envelope: CanvasNativeEnvelope,
}

/// What the native host can service right now.
///
/// The native host never advertises image-layer insertion or positioning, so SugarCubes never
/// routes generation results into input documents. Each flag must be true only while the host
/// can fulfil that family's operations.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CanvasNativeCapabilities {
    /// Output can be displayed through the shared surface.
    pub display_output: bool,
    /// `geometry.request` is serviced.
    pub geometry: bool,
    /// `edit.request` is serviced.
    pub editing: bool,
    /// `workflow.request` is serviced.
    pub workflow_surfaces: bool,
}

impl CanvasNativeCapabilities {
    /// Choose the narrowest shared capability form that states every flag exactly.
    fn to_wire(self) -> wire::CanvasCapabilities {
        if self.editing || self.workflow_surfaces {
            wire::CanvasCapabilities::Editing(wire::CanvasEditingCapabilities {
                display_output: self.display_output,
                insert_image_layer: false,
                position_image_layer: false,
                geometry: self.geometry,
                editing: self.editing,
                workflow_surfaces: self.workflow_surfaces,
            })
        } else if self.geometry {
            wire::CanvasCapabilities::Geometry(wire::CanvasGeometryCapabilities {
                display_output: self.display_output,
                insert_image_layer: false,
                position_image_layer: false,
                geometry: true,
            })
        } else {
            wire::CanvasCapabilities::Layer(wire::CanvasLayerCapabilities {
                display_output: self.display_output,
                insert_image_layer: false,
                position_image_layer: false,
            })
        }
    }
}

impl CanvasContextRequest {
    /// Decode the exact shared version-three context request.
    ///
    /// # Errors
    /// Rejects oversized, malformed, extended, duplicate or schema-invalid messages.
    pub fn decode(bytes: &[u8]) -> Result<Self, CanvasNativeError> {
        admit(bytes, MAX_CANVAS_CONTEXT_REQUEST_BYTES)?;
        let request: wire::CanvasReadRequest = parse(bytes)?;
        if !request.valid() {
            return Err(CanvasNativeError::Invalid);
        }
        Ok(Self { envelope: CanvasNativeEnvelope::new(request.session, request.request_id)? })
    }

    /// Borrow the request's public correlation.
    pub const fn envelope(&self) -> &CanvasNativeEnvelope {
        &self.envelope
    }

    /// Borrow the panel session that sent the request; it is not a credential.
    pub const fn session(&self) -> &crate::panel::PanelSession {
        self.envelope.session()
    }

    /// Return the request identity within its session.
    pub const fn request_id(&self) -> u64 {
        self.envelope.request_id()
    }

    /// Encode the host's capabilities and the active input document, if one is shown.
    ///
    /// The reply grants no layer, file, network, window or process privileges.
    /// # Errors
    /// Preserves schema or serialization failures and enforces the bounded reply size.
    pub fn reply(
        &self,
        capabilities: CanvasNativeCapabilities,
        document: Option<&CanvasDocument>,
    ) -> Result<CanvasNativeReplyPayload, CanvasNativeError> {
        let ReplyEnvelope { protocol, version, session, request_id } =
            ReplyEnvelope::of(&self.envelope);
        let reply = wire::CanvasReadReply {
            protocol,
            version,
            session,
            request_id,
            r#type: "context.reply".into(),
            context: wire::CanvasContext {
                canvas: wire::CanvasDescriptor {
                    id: "sugarsubstitute".into(),
                    label: "SugarSubstitute".into(),
                    capabilities: capabilities.to_wire(),
                },
                document: document.map(CanvasDocument::to_wire),
            },
        };
        self.envelope.payload(&reply, reply.valid(), MAX_CANVAS_CONTEXT_REPLY_BYTES)
    }
}
