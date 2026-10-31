// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Encode correlated surface outcomes using the generated closed shared shapes.

use super::{
    CanvasSurfaceMessageError, CanvasSurfaceOperation, CanvasSurfaceReplyStatus,
    CanvasSurfaceRequest, CanvasViewportState, MAX_SURFACE_MESSAGE_BYTES, wire,
};

impl CanvasSurfaceRequest {
    /// Encode an outcome bound to this exact session, request, attachment and revision.
    ///
    /// Successful viewport operations require [`Self::viewport_reply`] and their observed state.
    /// # Errors
    /// Rejects an invalid operation status, oversized text or serialization failure.
    pub fn reply(
        &self,
        status: CanvasSurfaceReplyStatus,
        message: &str,
    ) -> Result<Vec<u8>, CanvasSurfaceMessageError> {
        if (status == CanvasSurfaceReplyStatus::Ready
            && !matches!(self.operation(), CanvasSurfaceOperation::Prepare))
            || (status == CanvasSurfaceReplyStatus::Applied
                && matches!(
                    self.operation(),
                    CanvasSurfaceOperation::Prepare | CanvasSurfaceOperation::Viewport(_)
                ))
            || message.chars().count() > 512
        {
            return Err(CanvasSurfaceMessageError::Invalid);
        }
        let value = wire::CanvasSurfaceReply {
            protocol: "sugarcubes.canvas".into(),
            version: 3,
            session: self.session().as_str().into(),
            request_id: self.request_id(),
            r#type: "surface.reply".into(),
            address: self.address().wire().clone(),
            revision: self.revision(),
            status: status.as_str().into(),
            message: message.into(),
        };
        if !value.valid() {
            return Err(CanvasSurfaceMessageError::Invalid);
        }
        encode(&value)
    }

    /// Encode a successful viewport operation with the native owner's current snapshot.
    ///
    /// The snapshot describes presentation and grants no attachment or native authority.
    /// # Errors
    /// Rejects non-viewport requests, invalid state, oversized text or serialization failure.
    pub fn viewport_reply(
        &self,
        viewport: &CanvasViewportState,
        message: &str,
    ) -> Result<Vec<u8>, CanvasSurfaceMessageError> {
        if !matches!(self.operation(), CanvasSurfaceOperation::Viewport(_))
            || message.chars().count() > 512
        {
            return Err(CanvasSurfaceMessageError::Invalid);
        }
        let value = wire::CanvasSurfaceViewportReply {
            protocol: "sugarcubes.canvas".into(),
            version: 3,
            session: self.session().as_str().into(),
            request_id: self.request_id(),
            r#type: "surface.viewport.reply".into(),
            address: self.address().wire().clone(),
            revision: self.revision(),
            status: "applied".into(),
            message: message.into(),
            viewport: viewport.to_wire(),
        };
        if !value.valid() {
            return Err(CanvasSurfaceMessageError::Invalid);
        }
        encode(&value)
    }
}

/// Retain the complete surface-message byte bound after serialization.
pub(super) fn encode(value: &impl serde::Serialize) -> Result<Vec<u8>, CanvasSurfaceMessageError> {
    let bytes = serde_json::to_vec(value).map_err(CanvasSurfaceMessageError::Malformed)?;
    if bytes.len() > MAX_SURFACE_MESSAGE_BYTES {
        return Err(CanvasSurfaceMessageError::TooLarge);
    }
    Ok(bytes)
}
