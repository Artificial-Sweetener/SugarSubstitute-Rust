// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode and answer the shared native canvas protocol without granting host authority.
//!
//! SugarCubes addresses the desktop canvas through five request families on the version-three
//! `sugarcubes.canvas` envelope: `context.request`, `source.request`, `geometry.request`,
//! `edit.request` and `workflow.request`. Each decoder enforces an explicit byte bound before
//! JSON decoding, then the generated schema validators, then these owned domain types.
//! Requests up to 16 KiB arrive inline; larger ones arrive through the host's bulk channel and
//! still meet their family bound here.
//!
//! Every encoder returns schema-validated bytes bound to the exact session and request that
//! produced them. Notifications (`source.changed`, `edit.changed`) carry request id zero.
//! Nothing decoded here authenticates a page, resolves a path, or authorizes a document; the
//! panel session is public correlation data and the canvas owner checks every identity.

mod context;
mod edit;
mod edit_state;
mod geometry;
mod identity;
mod raster;
mod source;
#[cfg(test)]
mod tests;
mod wire;
mod workflow;
mod workflow_result;

pub use context::{CanvasContextRequest, CanvasNativeCapabilities};
pub use edit::{
    CanvasBrushOptions, CanvasBrushTool, CanvasCoverageModify, CanvasEditCommand, CanvasEditMode,
    CanvasEditRequest, CanvasHistoryAction, CanvasModifyOperation, CanvasModifyPhase,
    CanvasModifyTarget, CanvasSelectionAction, CanvasSessionAction, CanvasToolId,
    CanvasTransformAction,
};
pub use edit_state::{
    CanvasEditSelectionBounds, CanvasEditSession, CanvasEditSessionKind, CanvasEditState,
    CanvasEditTool,
};
pub use geometry::{
    CanvasGeometryCommand, CanvasGeometryContext, CanvasGeometryRequest, CanvasSelectionBounds,
};
pub use identity::{
    CanvasColor, CanvasDimensions, CanvasDocument, CanvasDocumentId, CanvasFailure,
    CanvasFailureMessage, CanvasFailureStatus, CanvasLabel, CanvasLayerId, CanvasLayerName,
    CanvasMaskKey, CanvasOpacity, CanvasRevision, CanvasSurfaceKey, CanvasWorkflowKey,
    MAX_CANVAS_DOCUMENT_SIDE,
};
pub use raster::{CanvasPixelFormat, CanvasPixelRaster};
pub use source::{
    CanvasCaptureSize, CanvasSourceCapture, CanvasSourceChange, CanvasSourceCommand,
    CanvasSourceContext, CanvasSourceLayer, CanvasSourceRequest, CanvasSourceSnapshot,
    CanvasSourceTarget, CanvasSourceTargetKind,
};
pub use workflow::{
    CanvasMaskChannel, CanvasResampling, CanvasResizeAnchor, CanvasWorkflowAppearance,
    CanvasWorkflowCommand, CanvasWorkflowMask, CanvasWorkflowRequest, CanvasWorkflowResizeMode,
    CanvasWorkflowRoute, CanvasWorkflowSurface, CanvasWorkflowSurfaceBase,
};
pub use workflow_result::{CanvasWorkflowSyncedMask, CanvasWorkflowSyncedSurface};

use crate::panel::PanelSession;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{error::Error, fmt};

/// Largest `context.request`, in UTF-8 bytes.
pub const MAX_CANVAS_CONTEXT_REQUEST_BYTES: usize = 4096;
/// Largest `source.request`; 64 capture targets with maximal layer ids fit.
pub const MAX_CANVAS_SOURCE_REQUEST_BYTES: usize = 64 * 1024;
/// Largest `geometry.request`.
pub const MAX_CANVAS_GEOMETRY_REQUEST_BYTES: usize = 4096;
/// Largest `edit.request`; every command is a few fixed fields.
pub const MAX_CANVAS_EDIT_REQUEST_BYTES: usize = 4096;
/// Largest `workflow.request`; a full sync of 64 surfaces with 32 masks each fits.
pub const MAX_CANVAS_WORKFLOW_REQUEST_BYTES: usize = 4 * 1024 * 1024;
/// Largest request of any family; decoding rejects larger input before parsing.
pub const MAX_CANVAS_NATIVE_REQUEST_BYTES: usize = MAX_CANVAS_WORKFLOW_REQUEST_BYTES;

/// Largest `context.reply` or `error.reply`.
pub const MAX_CANVAS_CONTEXT_REPLY_BYTES: usize = 4096;
/// Largest `source.reply`; one captured 64 MiB raster in base64 plus its JSON fits.
pub const MAX_CANVAS_SOURCE_REPLY_BYTES: usize = 96 * 1024 * 1024;
/// Largest `geometry.reply`.
pub const MAX_CANVAS_GEOMETRY_REPLY_BYTES: usize = 4096;
/// Largest `edit.reply` or `edit.changed`.
pub const MAX_CANVAS_EDIT_REPLY_BYTES: usize = 16 * 1024;
/// Largest `workflow.reply`; a full synced map of 64 surfaces fits.
pub const MAX_CANVAS_WORKFLOW_REPLY_BYTES: usize = 4 * 1024 * 1024;
/// Largest `source.changed` notification.
pub const MAX_CANVAS_SOURCE_CHANGED_BYTES: usize = 4096;

const PROTOCOL: &str = "sugarcubes.canvas";
const VERSION: u64 = 3;

/// The public correlation every native request carries; it is not a credential.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanvasNativeEnvelope {
    session: PanelSession,
    request_id: u64,
}

impl CanvasNativeEnvelope {
    /// Adopt the envelope fields of a generated request that already passed its validator.
    fn new(session: String, request_id: u64) -> Result<Self, CanvasNativeError> {
        if !(1..=2_147_483_647).contains(&request_id) {
            return Err(CanvasNativeError::Invalid);
        }
        let session = PanelSession::new(session).map_err(|_| CanvasNativeError::Invalid)?;
        Ok(Self { session, request_id })
    }

    /// Borrow the panel session that sent the request.
    pub const fn session(&self) -> &PanelSession {
        &self.session
    }

    /// Return the request identity, `1..=2147483647`, unique within its session's lifetime.
    pub const fn request_id(&self) -> u64 {
        self.request_id
    }

    /// Encode the shared cancellation `error.reply` for this request.
    ///
    /// The canvas owner sends this only after retiring the request's retained work.
    /// # Errors
    /// Preserves schema or serialization failures and enforces the reply bound.
    pub fn cancelled_reply(&self) -> Result<CanvasNativeReplyPayload, CanvasNativeError> {
        self.error_reply(&CanvasFailure::new(CanvasFailureStatus::Cancelled))
    }

    /// Encode a generic `error.reply` for a request that cannot use its family's reply.
    ///
    /// # Errors
    /// Preserves schema or serialization failures and enforces the reply bound.
    pub fn error_reply(
        &self,
        failure: &CanvasFailure,
    ) -> Result<CanvasNativeReplyPayload, CanvasNativeError> {
        let reply = wire::CanvasErrorReply {
            protocol: PROTOCOL.into(),
            version: VERSION,
            session: self.session.as_str().into(),
            request_id: self.request_id,
            r#type: "error.reply".into(),
            failure: failure.to_wire(),
        };
        self.payload(&reply, reply.valid(), MAX_CANVAS_CONTEXT_REPLY_BYTES)
    }

    /// Serialize a validated reply for this exact request within `maximum` bytes.
    fn payload(
        &self,
        reply: &impl Serialize,
        valid: bool,
        maximum: usize,
    ) -> Result<CanvasNativeReplyPayload, CanvasNativeError> {
        Ok(CanvasNativeReplyPayload {
            session: self.session.clone(),
            request_id: self.request_id,
            bytes: encode(reply, valid, maximum)?,
        })
    }
}

/// One decoded native request, dispatched to the canvas owner's matching entry point.
#[derive(Clone, Debug, PartialEq)]
pub enum CanvasNativeRequest {
    /// Read the canvas descriptor, capabilities and active document.
    Context(CanvasContextRequest),
    /// Read, capture or watch canvas pixel sources.
    Source(CanvasSourceRequest),
    /// Read or resize document geometry.
    Geometry(CanvasGeometryRequest),
    /// Drive the editing tools and read their state.
    Edit(CanvasEditRequest),
    /// Provision and present workflow input surfaces.
    Workflow(CanvasWorkflowRequest),
}

impl CanvasNativeRequest {
    /// Decode any native request family within its own byte bound.
    ///
    /// # Errors
    /// Rejects oversized, malformed, duplicate-field, extended, unknown or schema-invalid input.
    pub fn decode(bytes: &[u8]) -> Result<Self, CanvasNativeError> {
        admit(bytes, MAX_CANVAS_NATIVE_REQUEST_BYTES)?;
        #[derive(Deserialize)]
        struct Kind {
            #[serde(rename = "type")]
            kind: String,
        }
        let kind: Kind = parse(bytes)?;
        match kind.kind.as_str() {
            "context.request" => CanvasContextRequest::decode(bytes).map(Self::Context),
            "source.request" => CanvasSourceRequest::decode(bytes).map(Self::Source),
            "geometry.request" => CanvasGeometryRequest::decode(bytes).map(Self::Geometry),
            "edit.request" => CanvasEditRequest::decode(bytes).map(Self::Edit),
            "workflow.request" => CanvasWorkflowRequest::decode(bytes).map(Self::Workflow),
            _ => Err(CanvasNativeError::Invalid),
        }
    }

    /// Borrow the request's public correlation.
    pub const fn envelope(&self) -> &CanvasNativeEnvelope {
        match self {
            Self::Context(request) => request.envelope(),
            Self::Source(request) => request.envelope(),
            Self::Geometry(request) => request.envelope(),
            Self::Edit(request) => request.envelope(),
            Self::Workflow(request) => request.envelope(),
        }
    }

    /// Borrow the panel session that sent the request.
    pub const fn session(&self) -> &PanelSession {
        self.envelope().session()
    }

    /// Return the request identity within its session.
    pub const fn request_id(&self) -> u64 {
        self.envelope().request_id()
    }

    /// Encode this family's refusal: a failure reply, or for a context request a context that
    /// advertises no capabilities and no document, since context replies have no failure form.
    ///
    /// # Errors
    /// Preserves schema or serialization failures and enforces the family's reply bound.
    pub fn refusal_reply(
        &self,
        failure: &CanvasFailure,
    ) -> Result<CanvasNativeReplyPayload, CanvasNativeError> {
        match self {
            Self::Context(request) => request.reply(CanvasNativeCapabilities::default(), None),
            Self::Source(request) => request.failure_reply(failure),
            Self::Geometry(request) => request.failure_reply(failure),
            Self::Edit(request) => request.failure_reply(failure),
            Self::Workflow(request) => request.failure_reply(failure),
        }
    }
}

/// Schema-validated reply bytes bound to the session and request that produced them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanvasNativeReplyPayload {
    session: PanelSession,
    request_id: u64,
    bytes: Vec<u8>,
}

impl CanvasNativeReplyPayload {
    /// Borrow the session the reply answers.
    pub const fn session(&self) -> &PanelSession {
        &self.session
    }

    /// Return the request identity the reply answers.
    pub const fn request_id(&self) -> u64 {
        self.request_id
    }

    /// Borrow the exact UTF-8 JSON.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Transfer the exact UTF-8 JSON.
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

/// The host-initiated notifications of the native protocol.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CanvasNativeNotificationKind {
    /// `source.changed`: a watched source's content revision changed.
    SourceChanged,
    /// `edit.changed`: the editing state changed.
    EditChanged,
}

/// Schema-validated notification bytes for one panel session; each notification carries
/// complete state, so the newest of a kind supersedes an undelivered earlier one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanvasNativeNotificationPayload {
    session: PanelSession,
    kind: CanvasNativeNotificationKind,
    bytes: Vec<u8>,
}

impl CanvasNativeNotificationPayload {
    fn new(
        session: &PanelSession,
        kind: CanvasNativeNotificationKind,
        notification: &impl Serialize,
        valid: bool,
        maximum: usize,
    ) -> Result<Self, CanvasNativeError> {
        Ok(Self { session: session.clone(), kind, bytes: encode(notification, valid, maximum)? })
    }

    /// Borrow the session the notification addresses.
    pub const fn session(&self) -> &PanelSession {
        &self.session
    }

    /// Return which notification this is.
    pub const fn kind(&self) -> CanvasNativeNotificationKind {
        self.kind
    }

    /// Borrow the exact UTF-8 JSON.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Transfer the exact UTF-8 JSON.
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

/// Distinguish a bounded schema rejection from its actual JSON cause.
#[derive(Debug)]
pub enum CanvasNativeError {
    /// The payload exceeds its family's byte bound.
    TooLarge,
    /// JSON decoding or encoding failed.
    Malformed(serde_json::Error),
    /// The message or value does not satisfy the shared schema.
    Invalid,
}

impl fmt::Display for CanvasNativeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::TooLarge => "Native canvas message exceeds its byte bound",
            Self::Malformed(_) => "Native canvas message is malformed",
            Self::Invalid => "Native canvas message violates its schema",
        })
    }
}

impl Error for CanvasNativeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Malformed(cause) => Some(cause),
            Self::TooLarge | Self::Invalid => None,
        }
    }
}

/// Reject input above `maximum` before any JSON work.
fn admit(bytes: &[u8], maximum: usize) -> Result<(), CanvasNativeError> {
    if bytes.len() > maximum {
        return Err(CanvasNativeError::TooLarge);
    }
    Ok(())
}

/// Decode one closed generated shape, preserving the JSON cause.
fn parse<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, CanvasNativeError> {
    serde_json::from_slice(bytes).map_err(CanvasNativeError::Malformed)
}

/// Serialize a generated shape that passed its validator, within `maximum` bytes.
fn encode(
    value: &impl Serialize,
    valid: bool,
    maximum: usize,
) -> Result<Vec<u8>, CanvasNativeError> {
    if !valid {
        return Err(CanvasNativeError::Invalid);
    }
    let bytes = serde_json::to_vec(value).map_err(CanvasNativeError::Malformed)?;
    if bytes.len() > maximum {
        return Err(CanvasNativeError::TooLarge);
    }
    Ok(bytes)
}

/// The fixed envelope of a host reply addressed to `envelope`.
struct ReplyEnvelope {
    protocol: String,
    version: u64,
    session: String,
    request_id: u64,
}

impl ReplyEnvelope {
    fn of(envelope: &CanvasNativeEnvelope) -> Self {
        Self {
            protocol: PROTOCOL.into(),
            version: VERSION,
            session: envelope.session.as_str().into(),
            request_id: envelope.request_id,
        }
    }

    fn notification(session: &PanelSession) -> Self {
        Self {
            protocol: PROTOCOL.into(),
            version: VERSION,
            session: session.as_str().into(),
            request_id: 0,
        }
    }
}
