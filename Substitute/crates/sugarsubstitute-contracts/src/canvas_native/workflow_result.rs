// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Encode `workflow.reply`: the synced key map, activation, applied and failure outcomes.

use super::{
    CanvasDocument, CanvasFailure, CanvasLayerId, CanvasMaskKey, CanvasNativeError,
    CanvasNativeReplyPayload, CanvasSurfaceKey, CanvasWorkflowRequest,
    MAX_CANVAS_WORKFLOW_REPLY_BYTES, ReplyEnvelope, wire,
};

/// The host layer serving one synced mask; provisioned mask loaders bind to it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanvasWorkflowSyncedMask {
    /// SugarCubes' mask key.
    pub key: CanvasMaskKey,
    /// The host mask layer.
    pub layer: CanvasLayerId,
}

/// The host document serving one synced surface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanvasWorkflowSyncedSurface {
    /// SugarCubes' surface key.
    pub key: CanvasSurfaceKey,
    /// The host input document.
    pub document: CanvasDocument,
    /// Every mask of the surface, at most 32.
    pub masks: Vec<CanvasWorkflowSyncedMask>,
}

impl CanvasWorkflowRequest {
    /// Encode the key map for a sync, or for the one surface a resize changed.
    ///
    /// # Errors
    /// Rejects more than 64 surfaces or 32 masks per surface and preserves serialization
    /// failures and the reply bound.
    pub fn synced_reply(
        &self,
        surfaces: &[CanvasWorkflowSyncedSurface],
    ) -> Result<CanvasNativeReplyPayload, CanvasNativeError> {
        self.reply(wire::CanvasWorkflowResult::Synced {
            surfaces: surfaces
                .iter()
                .map(|surface| wire::CanvasWorkflowSyncedSurface {
                    key: surface.key.as_str().into(),
                    document: surface.document.to_wire(),
                    masks: surface
                        .masks
                        .iter()
                        .map(|mask| wire::CanvasWorkflowMaskLayer {
                            key: mask.key.as_str().into(),
                            layer_id: mask.layer.as_str().into(),
                        })
                        .collect(),
                })
                .collect(),
        })
    }

    /// Encode a completed activation.
    ///
    /// # Errors
    /// Preserves serialization failures.
    pub fn activated_reply(&self) -> Result<CanvasNativeReplyPayload, CanvasNativeError> {
        self.reply(wire::CanvasWorkflowResult::Activated {})
    }

    /// Encode a completed appearance or hover change.
    ///
    /// # Errors
    /// Preserves serialization failures.
    pub fn applied_reply(&self) -> Result<CanvasNativeReplyPayload, CanvasNativeError> {
        self.reply(wire::CanvasWorkflowResult::Applied {})
    }

    /// Encode a typed failure for any workflow command.
    ///
    /// # Errors
    /// Preserves serialization failures.
    pub fn failure_reply(
        &self,
        failure: &CanvasFailure,
    ) -> Result<CanvasNativeReplyPayload, CanvasNativeError> {
        let wire::CanvasFailure { status, message } = failure.to_wire();
        self.reply(wire::CanvasWorkflowResult::Failure { status, message })
    }

    fn reply(
        &self,
        result: wire::CanvasWorkflowResult,
    ) -> Result<CanvasNativeReplyPayload, CanvasNativeError> {
        let envelope = self.envelope();
        let ReplyEnvelope { protocol, version, session, request_id } = ReplyEnvelope::of(envelope);
        let reply = wire::CanvasWorkflowReply {
            protocol,
            version,
            session,
            request_id,
            r#type: "workflow.reply".into(),
            result,
        };
        envelope.payload(&reply, reply.valid(), MAX_CANVAS_WORKFLOW_REPLY_BYTES)
    }
}
