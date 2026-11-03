// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode `geometry.request` and encode its context, resize and failure replies.

use super::{
    CanvasDimensions, CanvasDocument, CanvasFailure, CanvasNativeEnvelope, CanvasNativeError,
    CanvasNativeReplyPayload, MAX_CANVAS_DOCUMENT_SIDE, MAX_CANVAS_GEOMETRY_REPLY_BYTES,
    MAX_CANVAS_GEOMETRY_REQUEST_BYTES, ReplyEnvelope, admit, parse, wire,
};

/// One `geometry.request` command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanvasGeometryCommand {
    /// Describe the active document's geometry and selection bounds.
    Context,
    /// Resize `document`, which must still carry its current geometry revision.
    Resize {
        /// The document as SugarCubes last saw it.
        document: CanvasDocument,
        /// The requested extent.
        size: CanvasDimensions,
    },
}

/// A validated `geometry.request` from one panel session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanvasGeometryRequest {
    envelope: CanvasNativeEnvelope,
    command: CanvasGeometryCommand,
}

/// A selection's bounding box in document pixels.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CanvasSelectionBounds {
    left: i32,
    top: i32,
    size: CanvasDimensions,
}

impl CanvasSelectionBounds {
    /// Adopt bounds whose origin lies within ±300000 document pixels.
    ///
    /// # Errors
    /// Rejects an origin outside the shared range.
    pub fn new(left: i32, top: i32, size: CanvasDimensions) -> Result<Self, CanvasNativeError> {
        let range = -(MAX_CANVAS_DOCUMENT_SIDE as i32)..=MAX_CANVAS_DOCUMENT_SIDE as i32;
        if !range.contains(&left) || !range.contains(&top) {
            return Err(CanvasNativeError::Invalid);
        }
        Ok(Self { left, top, size })
    }

    /// Return the left edge in document pixels.
    pub const fn left(self) -> i32 {
        self.left
    }

    /// Return the top edge in document pixels.
    pub const fn top(self) -> i32 {
        self.top
    }

    /// Return the extent in document pixels.
    pub const fn size(self) -> CanvasDimensions {
        self.size
    }
}

/// The answer to [`CanvasGeometryCommand::Context`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanvasGeometryContext {
    /// Whether the host services geometry at all.
    pub supported: bool,
    /// Whether [`CanvasGeometryCommand::Resize`] is serviced.
    pub resize: bool,
    /// Whether selection bounds are reported.
    pub selection_supported: bool,
    /// The active document, if one is shown.
    pub document: Option<CanvasDocument>,
    /// The current selection's bounds, if a selection exists.
    pub selection: Option<CanvasSelectionBounds>,
}

impl CanvasGeometryRequest {
    /// Decode the exact shared `geometry.request`.
    ///
    /// # Errors
    /// Rejects oversized, malformed, extended, duplicate or schema-invalid messages.
    pub fn decode(bytes: &[u8]) -> Result<Self, CanvasNativeError> {
        admit(bytes, MAX_CANVAS_GEOMETRY_REQUEST_BYTES)?;
        let request: wire::CanvasGeometryRequest = parse(bytes)?;
        if !request.valid() {
            return Err(CanvasNativeError::Invalid);
        }
        let command = match request.command {
            wire::CanvasGeometryCommand::Context {} => CanvasGeometryCommand::Context,
            wire::CanvasGeometryCommand::Resize { document, size } => {
                CanvasGeometryCommand::Resize {
                    document: CanvasDocument::from_wire(document)?,
                    size: CanvasDimensions::from_wire(size.width, size.height)?,
                }
            }
        };
        Ok(Self {
            envelope: CanvasNativeEnvelope::new(request.session, request.request_id)?,
            command,
        })
    }

    /// Borrow the request's public correlation.
    pub const fn envelope(&self) -> &CanvasNativeEnvelope {
        &self.envelope
    }

    /// Borrow the decoded command.
    pub const fn command(&self) -> &CanvasGeometryCommand {
        &self.command
    }

    /// Encode the answer to a context command.
    ///
    /// # Errors
    /// Preserves serialization failures and the reply bound.
    pub fn context_reply(
        &self,
        context: &CanvasGeometryContext,
    ) -> Result<CanvasNativeReplyPayload, CanvasNativeError> {
        self.reply(wire::CanvasGeometryResult::Context {
            supported: context.supported,
            resize: context.resize,
            selection_supported: context.selection_supported,
            document: context.document.as_ref().map(CanvasDocument::to_wire),
            selection: context.selection.map(|bounds| wire::CanvasSelectionBounds {
                left: bounds.left.into(),
                top: bounds.top.into(),
                width: bounds.size.width().into(),
                height: bounds.size.height().into(),
            }),
        })
    }

    /// Encode a completed resize with the document's new geometry revision and extent.
    ///
    /// # Errors
    /// Preserves serialization failures and the reply bound.
    pub fn resized_reply(
        &self,
        document: &CanvasDocument,
    ) -> Result<CanvasNativeReplyPayload, CanvasNativeError> {
        self.reply(wire::CanvasGeometryResult::Resized { document: document.to_wire() })
    }

    /// Encode a typed failure for any geometry command.
    ///
    /// # Errors
    /// Preserves serialization failures.
    pub fn failure_reply(
        &self,
        failure: &CanvasFailure,
    ) -> Result<CanvasNativeReplyPayload, CanvasNativeError> {
        let wire::CanvasFailure { status, message } = failure.to_wire();
        self.reply(wire::CanvasGeometryResult::Failure { status, message })
    }

    fn reply(
        &self,
        result: wire::CanvasGeometryResult,
    ) -> Result<CanvasNativeReplyPayload, CanvasNativeError> {
        let ReplyEnvelope { protocol, version, session, request_id } =
            ReplyEnvelope::of(&self.envelope);
        let reply = wire::CanvasGeometryReply {
            protocol,
            version,
            session,
            request_id,
            r#type: "geometry.reply".into(),
            result,
        };
        self.envelope.payload(&reply, reply.valid(), MAX_CANVAS_GEOMETRY_REPLY_BYTES)
    }
}
