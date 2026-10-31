// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode and correlate the shared surface operations without granting host authority.

use super::{
    CanvasFileReference, CanvasSurfaceAddress, CanvasSurfaceDiscoveryRequest,
    CanvasSurfaceGeometry, CanvasSurfacePresentationRequest, CanvasSurfacePreviewBegin,
    CanvasSurfacePreviewChunk, CanvasSurfacePreviewLease, CanvasViewportCommand, wire,
};
use crate::panel::PanelSession;
use serde::Deserialize;
use std::{error::Error, fmt};

/// Maximum encoded surface message before JSON parsing, in UTF-8 bytes.
pub const MAX_SURFACE_MESSAGE_BYTES: usize = 32 * 1024;

/// A validated surface request or cancellation from the existing canvas session.
#[derive(Clone, Debug, PartialEq)]
pub enum CanvasSurfaceMessage {
    /// Discover the runtime's advertised surface contract without creating an attachment.
    Discovery(CanvasSurfaceDiscoveryRequest),
    /// Enable or disable observed presentation changes for the current panel session.
    Presentation(CanvasSurfacePresentationRequest),
    /// One attachment operation, still requiring native navigation and revision authority.
    Request(Box<CanvasSurfaceRequest>),
    /// Cancel a previously admitted request in the same panel session.
    Cancel {
        /// Public correlation data, not an authorization token.
        session: PanelSession,
        /// The original request identity; cancellation does not allocate a new one.
        request_id: u64,
    },
}

/// One immutable operation and the complete identity of its originating attachment.
#[derive(Clone, Debug, PartialEq)]
pub struct CanvasSurfaceRequest {
    session: PanelSession,
    request_id: u64,
    address: CanvasSurfaceAddress,
    revision: u64,
    operation: CanvasSurfaceOperation,
}

/// Surface intent independent of native handles, filesystem paths and backend authority.
#[derive(Clone, Debug, PartialEq)]
pub enum CanvasSurfaceOperation {
    /// Prepare an initially hidden candidate without replacing the committed attachment.
    Prepare,
    /// Update the candidate or committed surface with client-space geometry.
    Geometry(CanvasSurfaceGeometry),
    /// Select an attributed Comfy result using a structured, untrusted file reference.
    Select {
        /// SugarCubes' selected result identity, retained only as correlation data.
        item_id: String,
        /// The Comfy adapter must validate the reference against its qualified backend.
        file: CanvasFileReference,
        /// An optional second result shown through a wipe under the same revision.
        comparison: Option<CanvasSurfaceComparison>,
    },
    /// Inspect or change the committed image viewport in attachment revision order.
    Viewport(CanvasViewportCommand),
    /// Admit one attributed, bounded JPEG transfer under this request's revision lease.
    PreviewBegin(CanvasSurfacePreviewBegin),
    /// Supply a bounded base64 chunk to an existing preview lease.
    PreviewChunk(CanvasSurfacePreviewChunk),
    /// Complete the addressed preview transfer without changing its original lease.
    PreviewEnd(CanvasSurfacePreviewLease),
    /// Withdraw only the identified preview lease and restore its final presentation.
    PreviewWithdraw(CanvasSurfacePreviewLease),
    /// Commit a prepared candidate as the current attachment.
    Commit,
    /// Release only the addressed candidate or committed attachment.
    Release,
}

/// A second attributed result selected for comparison with the primary selection.
#[derive(Clone, Debug, PartialEq)]
pub struct CanvasSurfaceComparison {
    /// SugarCubes' comparison result identity, retained only as correlation data.
    pub item_id: String,
    /// Structured, untrusted reference the Comfy adapter validates like the primary file.
    pub file: CanvasFileReference,
}

/// Stable host outcomes; successful preparation is distinct from applied operations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanvasSurfaceReplyStatus {
    /// A candidate is ready to be committed.
    Ready,
    /// A non-prepare operation was applied.
    Applied,
    /// The request or its revision no longer owns the target attachment.
    Stale,
    /// The original operation was canceled.
    Cancelled,
    /// This host cannot provide the requested surface.
    Unavailable,
    /// The surface operation failed.
    Error,
}

impl CanvasSurfaceMessage {
    /// Decode the exact closed shared shape without authorizing a native action.
    ///
    /// The caller must match `session` to the current top document and enforce attachment,
    /// request and revision ownership before performing an operation.
    /// # Errors
    /// Rejects oversized, malformed, unknown or schema-invalid messages.
    pub fn decode(bytes: &[u8]) -> Result<Self, CanvasSurfaceMessageError> {
        if bytes.len() > MAX_SURFACE_MESSAGE_BYTES {
            return Err(CanvasSurfaceMessageError::TooLarge);
        }
        #[derive(Deserialize)]
        struct Kind {
            #[serde(rename = "type")]
            kind: String,
        }
        let kind: Kind = parse(bytes)?;
        let request = match kind.kind.as_str() {
            "surface.capabilities.request" => {
                return CanvasSurfaceDiscoveryRequest::from_wire(parse(bytes)?)
                    .map(Self::Discovery);
            }
            "surface.presentation.request" => {
                return CanvasSurfacePresentationRequest::from_wire(parse(bytes)?)
                    .map(Self::Presentation);
            }
            "surface.preview.begin"
            | "surface.preview.chunk"
            | "surface.preview.end"
            | "surface.preview.withdraw" => super::preview::decode_request(&kind.kind, bytes)?,
            "surface.prepare" => {
                let value: wire::CanvasSurfacePrepareRequest = parse(bytes)?;
                if !value.valid() {
                    return Err(CanvasSurfaceMessageError::Invalid);
                }
                CanvasSurfaceRequest::new(
                    value.session,
                    value.request_id,
                    value.address,
                    value.revision,
                    CanvasSurfaceOperation::Prepare,
                )?
            }
            "surface.geometry" => {
                let value: wire::CanvasSurfaceGeometryRequest = parse(bytes)?;
                if !value.valid() {
                    return Err(CanvasSurfaceMessageError::Invalid);
                }
                let geometry = CanvasSurfaceGeometry::from_wire(value.geometry)
                    .ok_or(CanvasSurfaceMessageError::Invalid)?;
                CanvasSurfaceRequest::new(
                    value.session,
                    value.request_id,
                    value.address,
                    value.revision,
                    CanvasSurfaceOperation::Geometry(geometry),
                )?
            }
            "surface.select" => {
                let value: wire::CanvasSurfaceSelectRequest = parse(bytes)?;
                if !value.valid() {
                    return Err(CanvasSurfaceMessageError::Invalid);
                }
                let file = CanvasFileReference::from_wire(value.file)
                    .ok_or(CanvasSurfaceMessageError::Invalid)?;
                let comparison = value
                    .comparison
                    .map(|comparison| {
                        CanvasFileReference::from_wire(comparison.file)
                            .map(|file| CanvasSurfaceComparison {
                                item_id: comparison.item_id,
                                file,
                            })
                            .ok_or(CanvasSurfaceMessageError::Invalid)
                    })
                    .transpose()?;
                CanvasSurfaceRequest::new(
                    value.session,
                    value.request_id,
                    value.address,
                    value.revision,
                    CanvasSurfaceOperation::Select { item_id: value.item_id, file, comparison },
                )?
            }
            "surface.commit" => {
                let value: wire::CanvasSurfaceCommitRequest = parse(bytes)?;
                if !value.valid() {
                    return Err(CanvasSurfaceMessageError::Invalid);
                }
                CanvasSurfaceRequest::new(
                    value.session,
                    value.request_id,
                    value.address,
                    value.revision,
                    CanvasSurfaceOperation::Commit,
                )?
            }
            "surface.viewport.request" => {
                let value: wire::CanvasSurfaceViewportRequest = parse(bytes)?;
                if !value.valid() {
                    return Err(CanvasSurfaceMessageError::Invalid);
                }
                CanvasSurfaceRequest::new(
                    value.session,
                    value.request_id,
                    value.address,
                    value.revision,
                    CanvasSurfaceOperation::Viewport(CanvasViewportCommand::from_wire(
                        value.command,
                    )),
                )?
            }
            "surface.release" => {
                let value: wire::CanvasSurfaceReleaseRequest = parse(bytes)?;
                if !value.valid() {
                    return Err(CanvasSurfaceMessageError::Invalid);
                }
                CanvasSurfaceRequest::new(
                    value.session,
                    value.request_id,
                    value.address,
                    value.revision,
                    CanvasSurfaceOperation::Release,
                )?
            }
            "cancel" => {
                let value: wire::CanvasSurfaceCancelRequest = parse(bytes)?;
                if !value.valid() {
                    return Err(CanvasSurfaceMessageError::Invalid);
                }
                return Ok(Self::Cancel {
                    session: PanelSession::new(value.session)
                        .map_err(|_| CanvasSurfaceMessageError::Invalid)?,
                    request_id: value.request_id,
                });
            }
            _ => return Err(CanvasSurfaceMessageError::Unsupported),
        };
        Ok(Self::Request(Box::new(request)))
    }

    /// Borrow the originating panel session for independent navigation checks.
    pub fn session(&self) -> &PanelSession {
        match self {
            Self::Discovery(request) => request.session(),
            Self::Presentation(request) => request.session(),
            Self::Request(request) => &request.session,
            Self::Cancel { session, .. } => session,
        }
    }
}

impl CanvasSurfaceRequest {
    /// Adopt only schema-valid correlation data; callers still enforce actual ownership.
    pub(super) fn new(
        session: String,
        request_id: u64,
        address: wire::CanvasSurfaceAddress,
        revision: u64,
        operation: CanvasSurfaceOperation,
    ) -> Result<Self, CanvasSurfaceMessageError> {
        Ok(Self {
            session: PanelSession::new(session).map_err(|_| CanvasSurfaceMessageError::Invalid)?,
            request_id,
            address: CanvasSurfaceAddress::from_wire(address)
                .ok_or(CanvasSurfaceMessageError::Invalid)?,
            revision,
            operation,
        })
    }

    /// Borrow the public session label for independent current-navigation checks.
    pub fn session(&self) -> &PanelSession {
        &self.session
    }

    /// Return the request identity within this panel session.
    pub const fn request_id(&self) -> u64 {
        self.request_id
    }
    /// Borrow the complete untrusted attachment address.
    pub fn address(&self) -> &CanvasSurfaceAddress {
        &self.address
    }
    /// Return the monotonic operation revision reported for this attachment.
    pub const fn revision(&self) -> u64 {
        self.revision
    }
    /// Borrow the requested operation without transferring its source data.
    pub fn operation(&self) -> &CanvasSurfaceOperation {
        &self.operation
    }
}

impl CanvasSurfaceReplyStatus {
    /// Return the schema's stable wire spelling.
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Applied => "applied",
            Self::Stale => "stale",
            Self::Cancelled => "cancelled",
            Self::Unavailable => "unavailable",
            Self::Error => "error",
        }
    }
}

/// Distinguish admission, schema and JSON failures without discarding decoder causes.
#[derive(Debug)]
pub enum CanvasSurfaceMessageError {
    /// Encoded data exceeds the fixed admission bound.
    TooLarge,
    /// The JSON does not match a closed message shape.
    Malformed(serde_json::Error),
    /// The recognized shape violates shared schema bounds.
    Invalid,
    /// The message belongs to a different canvas operation.
    Unsupported,
}
impl fmt::Display for CanvasSurfaceMessageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::TooLarge => "Canvas surface message exceeds its byte bound",
            Self::Malformed(_) => "Canvas surface message is malformed",
            Self::Invalid => "Canvas surface message violates its schema",
            Self::Unsupported => "Canvas surface message type is unsupported",
        })
    }
}
impl Error for CanvasSurfaceMessageError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Malformed(cause) => Some(cause),
            _ => None,
        }
    }
}

/// Preserve each concrete JSON decoder error at the boundary.
pub(super) fn parse<T: serde::de::DeserializeOwned>(
    bytes: &[u8],
) -> Result<T, CanvasSurfaceMessageError> {
    serde_json::from_slice(bytes).map_err(CanvasSurfaceMessageError::Malformed)
}

#[cfg(test)]
mod tests;
