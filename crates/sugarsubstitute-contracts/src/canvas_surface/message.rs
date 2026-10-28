// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode and correlate the shared surface operations without granting host authority.

use super::{CanvasFileReference, CanvasSurfaceAddress, CanvasSurfaceGeometry, wire};
use crate::panel::PanelSession;
use serde::Deserialize;
use std::{error::Error, fmt};

/// Maximum encoded surface message before JSON parsing, in UTF-8 bytes.
pub const MAX_SURFACE_MESSAGE_BYTES: usize = 32 * 1024;

/// A validated surface request or cancellation from the existing canvas session.
#[derive(Clone, Debug, PartialEq)]
pub enum CanvasSurfaceMessage {
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
    },
    /// Commit a prepared candidate as the current attachment.
    Commit,
    /// Release only the addressed candidate or committed attachment.
    Release,
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
                CanvasSurfaceRequest::new(
                    value.session,
                    value.request_id,
                    value.address,
                    value.revision,
                    CanvasSurfaceOperation::Select { item_id: value.item_id, file },
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
            Self::Request(request) => &request.session,
            Self::Cancel { session, .. } => session,
        }
    }
}

impl CanvasSurfaceRequest {
    /// Adopt only schema-valid correlation data; callers still enforce actual ownership.
    fn new(
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

    /// Encode a response bound to this exact session, request, attachment and revision.
    /// # Errors
    /// Rejects an invalid status for preparation, an oversized message or serialization failure.
    pub fn reply(
        &self,
        status: CanvasSurfaceReplyStatus,
        message: &str,
    ) -> Result<Vec<u8>, CanvasSurfaceMessageError> {
        if (status == CanvasSurfaceReplyStatus::Ready
            && !matches!(self.operation, CanvasSurfaceOperation::Prepare))
            || (status == CanvasSurfaceReplyStatus::Applied
                && matches!(self.operation, CanvasSurfaceOperation::Prepare))
            || message.chars().count() > 512
        {
            return Err(CanvasSurfaceMessageError::Invalid);
        }
        let value = wire::CanvasSurfaceReply {
            protocol: "sugarcubes.canvas".into(),
            version: 3,
            session: self.session.as_str().into(),
            request_id: self.request_id,
            r#type: "surface.reply".into(),
            address: self.address.wire().clone(),
            revision: self.revision,
            status: status.as_str().into(),
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

impl CanvasSurfaceReplyStatus {
    /// Return the schema's stable wire spelling.
    const fn as_str(self) -> &'static str {
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
fn parse<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, CanvasSurfaceMessageError> {
    serde_json::from_slice(bytes).map_err(CanvasSurfaceMessageError::Malformed)
}

#[cfg(test)]
mod tests;
