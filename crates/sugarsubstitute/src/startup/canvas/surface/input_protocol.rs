// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Translate between the session's input documents and native protocol values: document and
//! layer identities, synced key maps, failures, appearance and resize modes. Identity text
//! follows the input-document owner's one mapping, so every message family names a document or
//! layer the same way.
use crate::native_canvas::session::inputs::{
    InitialOutcome, InputDocumentId, InputReply, ResizeAnchor, ResizeMode, SurfaceOutcome,
    SyncedSurface, layer_from_protocol_id, layer_protocol_id,
};
use std::error::Error;
use sugarsubstitute_canvas::{LayerId, MaskAppearance};
use sugarsubstitute_contracts::canvas_native::{
    CanvasColor, CanvasDimensions, CanvasDocument, CanvasDocumentId, CanvasFailure,
    CanvasFailureMessage, CanvasFailureStatus, CanvasLayerId, CanvasMaskKey, CanvasNativeError,
    CanvasOpacity, CanvasResampling, CanvasResizeAnchor, CanvasRevision, CanvasSurfaceKey,
    CanvasWorkflowResizeMode, CanvasWorkflowSyncedMask, CanvasWorkflowSyncedSurface,
};
use sugarsubstitute_image_ops::{CoverageResampling, Extent};

/// The protocol layer identity of one host layer.
pub(in crate::startup::canvas::surface) fn layer_key(
    layer: LayerId,
) -> Result<CanvasLayerId, Box<dyn Error>> {
    Ok(CanvasLayerId::new(layer_protocol_id(layer))?)
}

/// The host layer a protocol layer identity names, when it names one.
pub(in crate::startup::canvas::surface) fn layer_of(key: &CanvasLayerId) -> Option<LayerId> {
    layer_from_protocol_id(key.as_str())
}

/// The protocol document identity of one input document.
pub(in crate::startup::canvas::surface) fn document_id(
    document: InputDocumentId,
) -> Result<CanvasDocumentId, CanvasNativeError> {
    CanvasDocumentId::new(document.protocol_id())
}

/// The input document a protocol document identity names, when it names one.
pub(in crate::startup::canvas::surface) fn document_of(
    id: &CanvasDocumentId,
) -> Option<InputDocumentId> {
    InputDocumentId::from_protocol_id(id.as_str())
}

/// The protocol document for an input document at `extent`.
pub(in crate::startup::canvas::surface) fn canvas_document(
    document: InputDocumentId,
    extent: Extent,
) -> Result<CanvasDocument, CanvasNativeError> {
    Ok(CanvasDocument {
        id: document_id(document)?,
        geometry_revision: CanvasRevision::new(document.geometry(extent))?,
        size: CanvasDimensions::new(extent.width(), extent.height())?,
    })
}

/// The protocol document a settled surface reports.
fn synced_document(surface: &SyncedSurface) -> Result<CanvasDocument, CanvasNativeError> {
    Ok(CanvasDocument {
        id: document_id(surface.document)?,
        geometry_revision: CanvasRevision::new(surface.geometry.as_str())?,
        size: CanvasDimensions::new(surface.extent.width(), surface.extent.height())?,
    })
}

/// A failure with bounded developer detail.
pub(in crate::startup::canvas::surface) fn failure(
    status: CanvasFailureStatus,
    message: &str,
) -> CanvasFailure {
    let detail: String = message.chars().take(500).collect();
    CanvasFailure {
        status,
        message: CanvasFailureMessage::new(detail)
            .unwrap_or_else(|_| CanvasFailure::new(status).message),
    }
}

/// The key map of a settled sync or resize, or the failure that keeps it from being complete.
/// A mask file of the wrong size leaves its mask as it was and is reported by SugarCubes' own
/// pre-check; the canvas only records it.
pub(in crate::startup::canvas::surface) fn synced(
    reply: &InputReply,
) -> Result<Vec<CanvasWorkflowSyncedSurface>, CanvasFailure> {
    let error = |message: String| failure(CanvasFailureStatus::Error, &message);
    reply
        .surfaces
        .iter()
        .map(|outcome| {
            let surface = match outcome {
                SurfaceOutcome::Synced(surface) => surface,
                SurfaceOutcome::Failed { key, failure } => {
                    return Err(error(format!("Input surface {key} failed: {failure}")));
                }
            };
            let masks = surface
                .masks
                .iter()
                .map(|mask| {
                    match &mask.initial {
                        Some(InitialOutcome::Failed(cause)) => {
                            return Err(error(format!("Mask {} did not load: {cause}", mask.key)));
                        }
                        Some(InitialOutcome::Mismatch { expected, actual }) => tracing::warn!(
                            operation = "canvas_workflow_mask",
                            expected_width = expected.width(),
                            expected_height = expected.height(),
                            actual_width = actual.width(),
                            actual_height = actual.height(),
                            "Mask dimensions do not match; the mask was left as it was"
                        ),
                        Some(InitialOutcome::Loaded) | None => {}
                    }
                    Ok(CanvasWorkflowSyncedMask {
                        key: CanvasMaskKey::new(&*mask.key)
                            .map_err(|cause| error(cause.to_string()))?,
                        layer: layer_key(mask.layer).map_err(|cause| error(cause.to_string()))?,
                    })
                })
                .collect::<Result<_, _>>()?;
            Ok(CanvasWorkflowSyncedSurface {
                key: CanvasSurfaceKey::new(&*surface.key)
                    .map_err(|cause| error(cause.to_string()))?,
                document: synced_document(surface).map_err(|cause| error(cause.to_string()))?,
                masks,
            })
        })
        .collect()
}

pub(in crate::startup::canvas::surface) fn appearance(
    color: CanvasColor,
    opacity: CanvasOpacity,
) -> Result<MaskAppearance, Box<dyn Error>> {
    Ok(MaskAppearance::new([color.red, color.green, color.blue], opacity.get() as f32)?)
}

pub(in crate::startup::canvas::surface) fn extent(
    size: CanvasDimensions,
) -> Result<Extent, CanvasFailure> {
    Extent::new(size.width(), size.height())
        .map_err(|_| failure(CanvasFailureStatus::Error, "A blank surface size is invalid"))
}

pub(in crate::startup::canvas::surface) fn resize(mode: CanvasWorkflowResizeMode) -> ResizeMode {
    match mode {
        CanvasWorkflowResizeMode::Canvas(anchor) => ResizeMode::Canvas(match anchor {
            CanvasResizeAnchor::TopLeft => ResizeAnchor::TopLeft,
            CanvasResizeAnchor::Top => ResizeAnchor::Top,
            CanvasResizeAnchor::TopRight => ResizeAnchor::TopRight,
            CanvasResizeAnchor::Left => ResizeAnchor::Left,
            CanvasResizeAnchor::Center => ResizeAnchor::Center,
            CanvasResizeAnchor::Right => ResizeAnchor::Right,
            CanvasResizeAnchor::BottomLeft => ResizeAnchor::BottomLeft,
            CanvasResizeAnchor::Bottom => ResizeAnchor::Bottom,
            CanvasResizeAnchor::BottomRight => ResizeAnchor::BottomRight,
        }),
        CanvasWorkflowResizeMode::Scale(resampling) => ResizeMode::Scale(match resampling {
            CanvasResampling::Nearest => CoverageResampling::Nearest,
            CanvasResampling::Smooth => CoverageResampling::Smooth,
        }),
    }
}
