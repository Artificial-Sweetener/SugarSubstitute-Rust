// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! An edit keeps the shown backing and prepares only the pixels it changed.

use super::*;
use crate::native_canvas::session::preparation::frame_reuse;
use sugarsubstitute_canvas::{
    Document, DocumentEdit, Layer, LayerContent, LayerId, MaskAppearance,
};
use sugarsubstitute_contracts::execution::{MemoryLease, MemoryRelease};
use sugarsubstitute_image_ops::{Coverage, FULL_COVERAGE, Reconstruction, Region};

struct Released;
impl MemoryRelease for Released {
    fn release(&self, _: u64) {}
}

fn mask(coverage: Coverage) -> Result<Layer, Box<dyn Error>> {
    let appearance = MaskAppearance::new([255, 0, 0], 0.5)?;
    Ok(Layer::new(LayerId::new(2)?, LayerContent::Coverage { coverage, appearance })?)
}

fn shown(
    document: (u64, u64, u64),
    view: Viewport,
    reconstruction: Reconstruction,
) -> Result<Presented, Box<dyn Error>> {
    Ok(Presented {
        sampling_scale: view.zoom(),
        tier: BackingTier::Detail,
        coverage: BackingCoverage::new(view, [0; 2])?,
        view,
        reconstruction,
        document,
        source_revision: 0,
        comparison: None,
    })
}

#[test]
fn an_edit_repaints_its_mapped_region_and_other_documents_repaint_everything()
-> Result<(), Box<dyn Error>> {
    let memory = MemoryLease::new(64 * 1024 * 1024, Box::new(Released));
    let extent = Extent::new(2048, 1024)?;
    let mut document = Document::new(5, extent, &memory)?;
    let empty = Coverage::new(extent, 0, &memory)?;
    document.apply(DocumentEdit::Insert { index: 0, layer: mask(empty.clone())? })?;
    let before = document.snapshot();
    let mut painted = empty;
    painted.update(Region::new(1000, 500, 4, 4)?, &memory, |_, _, _| FULL_COVERAGE)?;
    document.apply(DocumentEdit::Replace(mask(painted)?))?;
    let after = document.snapshot();

    // Half scale: the edited tile 896..1024 × 384..512 maps to 448..512 × 192..256, plus the
    // filter margin of 11 device pixels on every side.
    let mut view = Viewport::new(Extent::new(1024, 512)?)?;
    view.zoom_at(0.5, [0.0; 2])?;
    let finished = Reconstruction::Finished(sugarsubstitute_image_ops::Filter::Lanczos3);
    let (before_key, after_key) =
        ((before.identity(), before.revision(), 0), (after.identity(), after.revision(), 0));
    let earlier = shown(before_key, view, finished)?;
    let kept = (before_key, before.clone());
    let reuse =
        frame_reuse(Some(&earlier), Some(&kept), &after, after_key, 0, None, finished, view)?;
    assert!(reuse.compatible);
    assert!(reuse.damage.blit().is_none());
    assert_eq!(
        reuse.damage.dirty().regions().collect::<Vec<_>>(),
        [Region::new(437, 181, 86, 86)?]
    );

    // Without the shown revision's snapshot, or for another document, everything is prepared.
    let reuse = frame_reuse(Some(&earlier), None, &after, after_key, 0, None, finished, view)?;
    assert!(!reuse.compatible);
    assert_eq!(reuse.damage.dirty().pixels(), view.surface().pixels());
    let other = shown((before.identity() + 1, before.revision(), 0), view, finished)?;
    let reuse = frame_reuse(Some(&other), Some(&kept), &after, after_key, 0, None, finished, view)?;
    assert!(!reuse.compatible);

    // The same revision keeps everything.
    let current = shown(after_key, view, finished)?;
    let shown_after = (after_key, after.clone());
    let reuse = frame_reuse(
        Some(&current),
        Some(&shown_after),
        &after,
        after_key,
        0,
        None,
        finished,
        view,
    )?;
    assert!(reuse.compatible && reuse.damage.dirty().is_empty());
    Ok(())
}
