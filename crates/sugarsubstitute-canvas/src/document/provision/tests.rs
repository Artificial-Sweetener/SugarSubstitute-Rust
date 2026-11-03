// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Host provisioning keeps every reachable state consistent; geometry changes stay undoable.
use super::*;
use crate::{Viewport, tests::harness};
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_image_ops::{
    AssetId, Evaluator, FULL_COVERAGE, Pixel, RasterSource, Reconstruction, Region,
};

fn run(
    test: impl FnOnce(&JobContext) -> Result<(), CanvasError> + Send + 'static,
) -> harness::TestResult {
    harness::run(16 * 1024 * 1024, |context| {
        test(context).map_err(|cause| ImageError::Source(Box::new(cause)))
    })
}

fn base(extent: Extent, asset: u8) -> Result<Layer, CanvasError> {
    Ok(Layer::new(LayerId::new(1)?, LayerContent::Raster { asset: AssetId([asset; 32]), extent })?
        .with_locked(true))
}

fn mask(id: u64, coverage: Coverage, color: [u8; 3]) -> Result<Layer, CanvasError> {
    let appearance = MaskAppearance::new(color, 0.5)?;
    Layer::new(LayerId::new(id)?, LayerContent::Coverage { coverage, appearance })?
        .with_name("Mask")
}

fn painted(extent: Extent, value: u16, memory: &MemoryLease) -> Result<Coverage, CanvasError> {
    let mut coverage = Coverage::new(extent, 0, memory)?;
    coverage.update(Region::new(0, 0, 4, 4)?, memory, |_, _, _| value)?;
    Ok(coverage)
}

fn coverage_of(snapshot: &DocumentSnapshot, id: u64) -> Option<Coverage> {
    snapshot.layers().iter().find(|layer| layer.id().get() == id).and_then(|layer| {
        match layer.content() {
            LayerContent::Coverage { coverage, .. } => Some(coverage.clone()),
            _ => None,
        }
    })
}

#[test]
fn provisioned_masks_survive_undo_and_keep_each_state_s_painting() -> harness::TestResult {
    run(|context| {
        let memory = context.memory();
        let extent = Extent::new(64, 64)?;
        let mut document = Document::new(9, extent, memory)?;
        document.apply(DocumentEdit::Insert { index: 0, layer: base(extent, 1)? })?;
        let blank = Coverage::new(extent, 0, memory)?;
        assert!(document.provision(&[Provision::Insert {
            index: 1,
            layer: mask(2, blank.clone(), [255, 0, 0])?
        }])?);
        let provisioned = document.snapshot().revision();
        // A person paints the mask twice.
        let first = painted(extent, 1000, memory)?;
        let second = painted(extent, 2000, memory)?;
        document.apply(DocumentEdit::Replace(mask(2, first.clone(), [255, 0, 0])?))?;
        document.apply(DocumentEdit::Replace(mask(2, second.clone(), [255, 0, 0])?))?;
        // The workflow recolours and renames it, and adds another mask.
        let appearance = MaskAppearance::new([0, 0, 255], 0.25)?;
        assert!(document.provision(&[
            Provision::Appearance { layer: LayerId::new(2)?, appearance },
            Provision::Rename { layer: LayerId::new(2)?, name: Arc::from("Region") },
            Provision::Insert { index: 9, layer: mask(3, blank.clone(), [0, 255, 0])? },
        ])?);
        let current = document.snapshot();
        assert_eq!(current.layers().len(), 3, "an out-of-range index lands on top");
        assert_eq!(coverage_of(&current, 2), Some(second));
        assert!(document.undo()?);
        let undone = document.snapshot();
        assert_eq!(coverage_of(&undone, 2), Some(first), "painting history is kept");
        assert_eq!(undone.layers()[1].name(), "Region");
        assert!(matches!(
            undone.layers()[1].content(),
            LayerContent::Coverage { appearance: kept, .. } if *kept == appearance
        ));
        assert!(coverage_of(&undone, 3).is_some(), "undo never removes a provisioned mask");
        assert!(document.undo()?);
        assert_eq!(coverage_of(&document.snapshot(), 2), Some(blank.clone()));
        // Before the base was inserted the mask was provisioned too.
        assert!(document.undo()?);
        assert_eq!(document.snapshot().layers().len(), 2);
        assert!(!document.undo()?, "provisioning added no history step");
        assert!(document.snapshot().revision() > provisioned);
        assert!(document.redo()?);
        // Removal reaches redo states too.
        assert!(document.provision(&[Provision::Remove(LayerId::new(3)?)])?);
        assert!(document.redo()?);
        assert!(coverage_of(&document.snapshot(), 3).is_none());
        assert!(!document.provision(&[Provision::Remove(LayerId::new(3)?)])?, "no-op");
        // A change the current content cannot take is rejected without any effect.
        let revision = document.snapshot().revision();
        let wrong = Coverage::new(Extent::new(8, 8)?, 0, memory)?;
        assert!(
            document
                .provision(&[Provision::Insert { index: 2, layer: mask(4, wrong, [1, 2, 3])? }])
                .is_err()
        );
        assert_eq!(document.snapshot().revision(), revision);
        Ok(())
    })
}

#[test]
fn reframing_is_undoable_and_states_at_another_extent_cannot_take_provisioned_content()
-> harness::TestResult {
    run(|context| {
        let memory = context.memory();
        let (small, large) = (Extent::new(32, 32)?, Extent::new(48, 40)?);
        let mut document = Document::new(4, small, memory)?;
        document.apply(DocumentEdit::Insert { index: 0, layer: base(small, 1)? })?;
        document.apply(DocumentEdit::Select(Some(Coverage::new(small, FULL_COVERAGE, memory)?)))?;
        assert!(document.reframe(large, &[base(large, 1)?])?);
        let reframed = document.snapshot();
        assert_eq!((reframed.extent(), reframed.selection()), (large, None));
        assert!(document.undo()?);
        assert_eq!(document.snapshot().extent(), small);
        assert!(document.redo()?);
        assert!(document.reframe(large, &[base(large, 1)?, base(large, 2)?]).is_err());
        assert!(document.reframe(small, &[base(large, 1)?]).is_err());
        // A base image replaced at the current extent cannot reach the smaller states.
        let replacement = LayerContent::Raster { asset: AssetId([5; 32]), extent: large };
        assert!(document.provision(&[Provision::Content {
            layer: LayerId::new(1)?,
            content: replacement.clone()
        }])?);
        assert_eq!(document.snapshot().layers()[0].content(), &replacement);
        assert!(document.snapshot().layers()[0].locked());
        assert!(!document.undo()?, "states at another extent were discarded");
        Ok(())
    })
}

#[test]
fn reloading_replaces_extent_and_content_and_discards_history() -> harness::TestResult {
    run(|context| {
        let memory = context.memory();
        let (small, large) = (Extent::new(16, 16)?, Extent::new(30, 20)?);
        let mut document = Document::new(5, small, memory)?;
        document.apply(DocumentEdit::Insert { index: 0, layer: base(small, 1)? })?;
        let blank = Coverage::new(large, 0, memory)?;
        let layers = [base(large, 2)?, mask(2, blank, [9, 9, 9])?];
        assert!(document.reload(large, &layers)?);
        let reloaded = document.snapshot();
        assert_eq!((reloaded.extent(), reloaded.layers()), (large, &layers[..]));
        assert!(!document.undo()?);
        assert!(!document.reload(large, &layers)?, "identical content is a no-op");
        assert!(document.reload(large, &[layers[0].clone(), layers[0].clone()]).is_err());
        Ok(())
    })
}

/// Masks need no image source; any read would be a defect.
struct NoSource;
impl RasterSource for NoSource {
    fn read(
        &mut self,
        _: AssetId,
        _: Region,
        _: &mut [Pixel],
        _: &JobContext,
    ) -> Result<(), ImageError> {
        Err(ImageError::Invalid("Mask-only documents read no image source"))
    }
}

#[test]
fn a_highlight_lightens_its_overlay_without_history_and_damages_only_that_mask()
-> harness::TestResult {
    run(|context| {
        let memory = context.memory();
        let extent = Extent::new(64, 32)?;
        let mut document = Document::new(3, extent, memory)?;
        let mut coverage = Coverage::new(extent, 0, memory)?;
        coverage.update(Region::new(0, 0, 10, 10)?, memory, |_, _, _| FULL_COVERAGE)?;
        let appearance = MaskAppearance::new([255, 0, 0], 1.0)?;
        let layer = Layer::new(LayerId::new(2)?, LayerContent::Coverage { coverage, appearance })?;
        document.apply(DocumentEdit::Insert { index: 0, layer })?;
        let plain = document.snapshot();
        assert!(document.set_highlight(Some(LayerId::new(2)?))?);
        assert!(!document.set_highlight(Some(LayerId::new(2)?))?);
        assert!(document.set_highlight(Some(LayerId::new(7)?)).is_err());
        let lit = document.snapshot();
        assert_eq!(lit.highlight(), Some(LayerId::new(2)?));
        assert!(lit.revision() > plain.revision());
        assert_eq!(
            lit.damage_since(&plain).regions().collect::<Vec<_>>(),
            [Region::new(0, 0, 10, 10)?]
        );
        let compiled = lit.compile_view(
            Viewport::new(extent)?,
            None,
            Pixel::linear(0.0, 0.0, 0.0, 1.0)?,
            Reconstruction::Finished(sugarsubstitute_image_ops::Filter::Nearest),
            memory,
        )?;
        let (graph, output) = compiled.into_parts();
        let mut evaluator = Evaluator::new(graph, NoSource, 16, memory)?;
        let pixel = evaluator.tile(output, 0, 0, context)?.pixel(2, 2);
        let linear = |value: u8| ((f32::from(value) / 255.0 + 0.055) / 1.055).powf(2.4);
        assert_eq!(pixel, Some(Pixel::linear(1.0, linear(89), linear(89), 1.0)?));
        // Highlight is presentation: undo history is untouched and keeps the emphasis.
        assert!(document.undo()?);
        assert_eq!(document.snapshot().layers().len(), 0);
        assert_eq!(document.snapshot().highlight(), None, "the emphasized layer is gone");
        assert!(document.redo()?);
        assert_eq!(document.snapshot().highlight(), None);
        assert!(document.set_highlight(Some(LayerId::new(2)?))?);
        let renamed = document.snapshot().layers()[0].clone().with_name("Hovered")?;
        document.apply(DocumentEdit::Replace(renamed))?;
        assert!(document.undo()?);
        assert_eq!(document.snapshot().highlight(), Some(LayerId::new(2)?), "kept across undo");
        assert!(document.set_highlight(None)?);
        Ok(())
    })
}
