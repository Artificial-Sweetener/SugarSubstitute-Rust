// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove that a transient gesture uses one document history and rejects obsolete custody.
use super::*;
use crate::tests::harness;
use sugarsubstitute_image_ops::{AssetId, ImageError};

#[test]
fn memory_pressure_preserves_the_last_valid_staged_branch_and_can_retry() -> harness::TestResult {
    execute(|memory| {
        let extent = Extent::new(100, 100)?;
        let mut doc = Document::new(1, extent, memory)?;
        let original = layer(extent)?;
        let mut edit = doc.begin_edit(EditSessionId::new(1)?);
        edit.stage(DocumentEdit::Insert { index: 0, layer: original.clone() })?;
        let available = memory
            .claim(memory.bytes())
            .err()
            .ok_or(CanvasError::Invalid("Expected already-claimed document memory"))?
            .available_bytes;
        let blocker = memory.claim(available).map_err(ImageError::from)?;
        let replacement = original.clone().with_opacity(0.5)?;
        assert!(edit.stage(DocumentEdit::Replace(replacement.clone())).is_err());
        assert_eq!(edit.preview().layers(), &[original]);
        assert_eq!(doc.snapshot().revision(), 1);
        drop(blocker);
        edit.stage(DocumentEdit::Replace(replacement.clone()))?;
        assert!(doc.commit_edit(edit)?);
        assert_eq!(doc.snapshot().layers(), &[replacement]);
        Ok(())
    })
}
fn execute(
    action: impl FnOnce(&MemoryLease) -> Result<(), CanvasError> + Send + 'static,
) -> harness::TestResult {
    harness::run(2 * 1024 * 1024, |context| {
        action(context.memory()).map_err(|cause| ImageError::Source(Box::new(cause)))
    })
}
fn layer(extent: Extent) -> Result<Layer, CanvasError> {
    Layer::new(LayerId::new(1)?, LayerContent::Raster { asset: AssetId([1; 32]), extent })
}
#[test]
fn a_multi_update_gesture_publishes_once_and_undo_redo_restore_whole_changes() -> harness::TestResult
{
    execute(|memory| {
        let extent = Extent::new(100, 100)?;
        let mut doc = Document::new(1, extent, memory)?;
        let original = layer(extent)?;
        let mut edit = doc.begin_edit(EditSessionId::new(1)?);
        edit.stage(DocumentEdit::Insert { index: 0, layer: original.clone() })?;
        edit.stage(DocumentEdit::Replace(original.with_opacity(0.5)?))?;
        assert!(doc.snapshot().layers().is_empty());
        assert_eq!(edit.preview().layers()[0].opacity(), 0.5);
        assert_eq!(edit.preview().base().revision(), doc.snapshot().revision());
        assert!(doc.commit_edit(edit)?);
        assert_eq!(doc.snapshot().revision(), 2);
        assert!(doc.undo()?);
        assert!(doc.snapshot().layers().is_empty());
        assert!(!doc.undo()?);
        assert!(doc.redo()?);
        assert_eq!(doc.snapshot().layers()[0].opacity(), 0.5);
        Ok(())
    })
}
#[test]
fn cancelled_invalid_and_noop_gestures_preserve_document_revision_and_redo() -> harness::TestResult
{
    execute(|memory| {
        let extent = Extent::new(100, 100)?;
        let mut doc = Document::new(1, extent, memory)?;
        let original = layer(extent)?;
        doc.apply(DocumentEdit::Insert { index: 0, layer: original.clone() })?;
        doc.undo()?;
        let revision = doc.snapshot().revision();
        let mut edit = doc.begin_edit(EditSessionId::new(1)?);
        edit.stage(DocumentEdit::Insert { index: 0, layer: original.clone() })?;
        assert!(edit.stage(DocumentEdit::Insert { index: 0, layer: original.clone() }).is_err());
        assert_eq!(edit.preview().layers(), &[original]);
        drop(edit);
        let noop = doc.begin_edit(EditSessionId::new(2)?);
        assert!(!doc.commit_edit(noop)?);
        assert_eq!(doc.snapshot().revision(), revision);
        assert!(doc.redo()?);
        Ok(())
    })
}
#[test]
fn stale_and_foreign_sessions_cannot_publish_even_with_reused_document_identity()
-> harness::TestResult {
    execute(|memory| {
        let extent = Extent::new(100, 100)?;
        let mut doc = Document::new(1, extent, memory)?;
        let mut other = Document::new(1, extent, memory)?;
        let original = layer(extent)?;
        let mut foreign = doc.begin_edit(EditSessionId::new(1)?);
        foreign.stage(DocumentEdit::Insert { index: 0, layer: original.clone() })?;
        assert!(other.commit_edit(foreign).is_err());
        assert!(other.snapshot().layers().is_empty());
        let mut stale = doc.begin_edit(EditSessionId::new(2)?);
        stale.stage(DocumentEdit::Insert { index: 0, layer: original.clone() })?;
        doc.apply(DocumentEdit::Insert { index: 0, layer: original.clone() })?;
        assert!(doc.commit_edit(stale).is_err());
        assert_eq!(doc.snapshot().revision(), 2);
        assert_eq!(doc.snapshot().layers(), &[original]);
        Ok(())
    })
}
