// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode bounded preview transfer intent without assembling bytes or granting authority.

use super::{
    CanvasSurfaceMessageError, CanvasSurfaceOperation, CanvasSurfacePreviewFormat,
    CanvasSurfaceRequest, message::parse, wire,
};

mod admission;

/// Maximum encoded preview length admitted by the shared transfer schema, in bytes.
pub const MAX_SURFACE_PREVIEW_BYTES: usize = 64 * 1024 * 1024;
/// Maximum decoded chunk length; the native lease owner checks transfer cursors.
pub const MAX_SURFACE_PREVIEW_CHUNK_BYTES: usize = 8192;
/// Fixed native transfer deadline measured from begin admission, in milliseconds.
pub const SURFACE_PREVIEW_LEASE_MILLISECONDS: u64 = 30_000;

/// Schema-valid attribution and token grammar; applications qualify actual workflow ownership.
#[derive(Clone, Debug, PartialEq)]
pub struct CanvasSurfacePreviewIdentity(wire::CanvasSurfacePreviewIdentity);

/// One encoded preview declaration; its enclosing request revision identifies the lease.
#[derive(Clone, Debug, PartialEq)]
pub struct CanvasSurfacePreviewBegin {
    identity: CanvasSurfacePreviewIdentity,
    byte_length: u64,
    format: CanvasSurfacePreviewFormat,
}

/// One bounded canonical decoded chunk; native admission verifies exact transfer offsets.
#[derive(Clone, Debug, PartialEq)]
pub struct CanvasSurfacePreviewChunk {
    lease: CanvasSurfacePreviewLease,
    offset: u64,
    data: Vec<u8>,
}

/// A preview token and its original begin revision, independent of later camera revisions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanvasSurfacePreviewLease {
    token: String,
    revision: u64,
}

impl CanvasSurfacePreviewIdentity {
    /// Return the reported workflow generation, which can begin at zero.
    pub const fn generation(&self) -> u64 {
        self.0.generation
    }
    /// Borrow the untrusted prompt attribution.
    pub fn prompt_id(&self) -> &str {
        &self.0.prompt_id
    }
    /// Borrow the untrusted source-node attribution.
    pub fn source_id(&self) -> &str {
        &self.0.source_id
    }
    /// Return the positive workflow run order.
    pub const fn run_order(&self) -> u64 {
        self.0.run_order
    }
    /// Return the positive preview sequence within its workflow attribution.
    pub const fn sequence(&self) -> u64 {
        self.0.sequence
    }
    /// Borrow the bounded token; it provides correlation rather than native authority.
    pub fn token(&self) -> &str {
        &self.0.token
    }
}

impl CanvasSurfacePreviewBegin {
    /// Borrow attribution for independent workflow and source qualification.
    pub fn identity(&self) -> &CanvasSurfacePreviewIdentity {
        &self.identity
    }
    /// Return the declared complete encoded length, from one byte through 64 MiB.
    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }
    /// Return the declared encoding; runtime capabilities determine whether it is supported.
    pub const fn format(&self) -> CanvasSurfacePreviewFormat {
        self.format
    }
}

impl CanvasSurfacePreviewChunk {
    /// Borrow the preview token without changing its original lease identity.
    pub fn token(&self) -> &str {
        self.lease.token()
    }
    /// Return the original begin revision, which remains stable during camera changes.
    pub const fn lease_revision(&self) -> u64 {
        self.lease.lease_revision()
    }
    /// Return the reported decoded-byte offset within the transfer.
    pub const fn offset(&self) -> u64 {
        self.offset
    }
    /// Borrow canonical decoded bytes, bounded to one through 8192 bytes.
    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

impl CanvasSurfacePreviewLease {
    /// Borrow the preview token, independently of attachment and navigation authority.
    pub fn token(&self) -> &str {
        &self.token
    }
    /// Return the original begin revision, independently of the enclosing operation revision.
    pub const fn lease_revision(&self) -> u64 {
        self.revision
    }
}

/// Translate each exact generated preview shape after validation, retaining decoder causes.
pub(super) fn decode_request(
    kind: &str,
    bytes: &[u8],
) -> Result<CanvasSurfaceRequest, CanvasSurfaceMessageError> {
    match kind {
        "surface.preview.begin" => {
            let value: wire::CanvasSurfacePreviewBeginRequest = parse(bytes)?;
            if !value.valid() || !admission::valid_identity(&value.preview) {
                return Err(CanvasSurfaceMessageError::Invalid);
            }
            let format = match value.format.as_str() {
                "png" => CanvasSurfacePreviewFormat::Png,
                "jpeg" => CanvasSurfacePreviewFormat::Jpeg,
                _ => return Err(CanvasSurfaceMessageError::Invalid),
            };
            CanvasSurfaceRequest::new(
                value.session,
                value.request_id,
                value.address,
                value.revision,
                CanvasSurfaceOperation::PreviewBegin(CanvasSurfacePreviewBegin {
                    identity: CanvasSurfacePreviewIdentity(value.preview),
                    byte_length: value.byte_length,
                    format,
                }),
            )
        }
        "surface.preview.chunk" => {
            let value: wire::CanvasSurfacePreviewChunkRequest = parse(bytes)?;
            if !value.valid() {
                return Err(CanvasSurfaceMessageError::Invalid);
            }
            let data =
                admission::decode_chunk(&value.data).ok_or(CanvasSurfaceMessageError::Invalid)?;
            CanvasSurfaceRequest::new(
                value.session,
                value.request_id,
                value.address,
                value.revision,
                CanvasSurfaceOperation::PreviewChunk(CanvasSurfacePreviewChunk {
                    lease: CanvasSurfacePreviewLease {
                        token: value.token,
                        revision: value.lease_revision,
                    },
                    offset: value.offset,
                    data,
                }),
            )
        }
        "surface.preview.end" => {
            let value: wire::CanvasSurfacePreviewEndRequest = parse(bytes)?;
            if !value.valid() {
                return Err(CanvasSurfaceMessageError::Invalid);
            }
            CanvasSurfaceRequest::new(
                value.session,
                value.request_id,
                value.address,
                value.revision,
                CanvasSurfaceOperation::PreviewEnd(CanvasSurfacePreviewLease {
                    token: value.token,
                    revision: value.lease_revision,
                }),
            )
        }
        "surface.preview.withdraw" => {
            let value: wire::CanvasSurfacePreviewWithdrawRequest = parse(bytes)?;
            if !value.valid() {
                return Err(CanvasSurfaceMessageError::Invalid);
            }
            CanvasSurfaceRequest::new(
                value.session,
                value.request_id,
                value.address,
                value.revision,
                CanvasSurfaceOperation::PreviewWithdraw(CanvasSurfacePreviewLease {
                    token: value.token,
                    revision: value.lease_revision,
                }),
            )
        }
        _ => Err(CanvasSurfaceMessageError::Unsupported),
    }
}

#[cfg(test)]
pub(super) mod tests;
