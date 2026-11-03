// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Document edits that shape tools and selection commands compute inside owned jobs.
use crate::{CanvasError, DocumentEdit, DocumentSnapshot, Layer, LayerContent, LayerId};
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_image_ops::{CombineMode, Coverage, CoverageAffine};

/// Join incoming coverage into the snapshot's selection. A result that covers nothing
/// deselects, so a selection always covers something.
pub(super) fn select(
    base: &DocumentSnapshot,
    incoming: &Coverage,
    mode: CombineMode,
    context: &JobContext,
) -> Result<DocumentEdit, CanvasError> {
    let combined = match (base.selection(), mode) {
        (Some(current), _) => current.combine(incoming, mode, context)?,
        (None, CombineMode::Replace | CombineMode::Add) => incoming.clone(),
        (None, CombineMode::Subtract | CombineMode::Intersect) => {
            return Ok(DocumentEdit::Select(None));
        }
    };
    Ok(DocumentEdit::Select(nonempty(combined)))
}

/// Replace the selection with `change(selection)`.
/// # Errors
/// Rejects a snapshot without a selection.
pub(super) fn reshape_selection(
    base: &DocumentSnapshot,
    change: impl FnOnce(&Coverage) -> Result<Coverage, CanvasError>,
) -> Result<DocumentEdit, CanvasError> {
    let selection = base.selection().ok_or(CanvasError::Invalid("Nothing is selected"))?;
    Ok(DocumentEdit::Select(nonempty(change(selection)?)))
}

/// Replace a mask layer's coverage with `change(coverage)`, keeping its appearance.
/// # Errors
/// Rejects a target that is missing, locked or not a mask.
pub(super) fn reshape_mask(
    base: &DocumentSnapshot,
    target: LayerId,
    change: impl FnOnce(&Coverage) -> Result<Coverage, CanvasError>,
) -> Result<DocumentEdit, CanvasError> {
    let layer = editable_mask(base, target).ok_or(CanvasError::Invalid("No editable mask"))?;
    let LayerContent::Coverage { coverage, appearance } = layer.content() else {
        return Err(CanvasError::Invalid("No editable mask"));
    };
    let coverage = change(coverage)?;
    Ok(DocumentEdit::Replace(
        layer.clone().with_content(LayerContent::Coverage { coverage, appearance: *appearance }),
    ))
}

/// Lift the selected pixels of a mask and land them under `affine`, as CuteCanvas places a
/// floating fragment: the fragment is the mask premultiplied by the selection, the area it
/// leaves is cut from the mask unless copying, and it lands over the mask through the moved
/// selection, which becomes the document's selection. One atomic step.
/// # Errors
/// Rejects a snapshot without a selection or editable mask and preserves job failures.
pub(super) fn float(
    base: &DocumentSnapshot,
    mask: LayerId,
    affine: CoverageAffine,
    copy: bool,
    context: &JobContext,
) -> Result<DocumentEdit, CanvasError> {
    let selection = base.selection().ok_or(CanvasError::Invalid("Nothing is selected"))?;
    let mut moved = None;
    let reshaped = reshape_mask(base, mask, |coverage| {
        let fragment = coverage.combine(selection, CombineMode::Intersect, context)?;
        let kept = if copy {
            coverage.clone()
        } else {
            coverage.combine(selection, CombineMode::Subtract, context)?
        };
        let alpha = selection.transform(affine, context)?;
        let placed = kept.over(&fragment.transform(affine, context)?, &alpha, context)?;
        moved = Some(alpha);
        Ok(placed)
    })?;
    Ok(DocumentEdit::Group(vec![reshaped, DocumentEdit::Select(moved.and_then(nonempty))]))
}

/// A whole mask under `affine`.
/// # Errors
/// Rejects a missing or locked mask and preserves job failures.
pub(super) fn transform_mask(
    base: &DocumentSnapshot,
    mask: LayerId,
    affine: CoverageAffine,
    context: &JobContext,
) -> Result<DocumentEdit, CanvasError> {
    reshape_mask(base, mask, |coverage| Ok(coverage.transform(affine, context)?))
}

/// The unlocked mask layer with this identity.
pub(super) fn editable_mask(document: &DocumentSnapshot, target: LayerId) -> Option<&Layer> {
    document.layers().iter().find(|layer| {
        layer.id() == target
            && !layer.locked()
            && matches!(layer.content(), LayerContent::Coverage { .. })
    })
}

fn nonempty(coverage: Coverage) -> Option<Coverage> {
    coverage.bounds().map(|_| coverage)
}
