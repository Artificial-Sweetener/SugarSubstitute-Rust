// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Mask layers, the document selection and the damage their edits cause.
use super::harness;
use crate::*;
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_image_ops::{
    AssetId, Coverage, Evaluator, Extent, FULL_COVERAGE, ImageError, Path, Pixel, RasterSource,
    Reconstruction, Region,
};

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

fn run(
    test: impl FnOnce(&JobContext) -> Result<(), CanvasError> + Send + 'static,
) -> harness::TestResult {
    harness::run(16 * 1024 * 1024, |context| {
        test(context).map_err(|cause| ImageError::Source(Box::new(cause)))
    })
}

fn mask_layer(id: u64, coverage: Coverage) -> Result<Layer, CanvasError> {
    let appearance = MaskAppearance::new([255, 0, 0], 0.5)?;
    Layer::new(LayerId::new(id)?, LayerContent::Coverage { coverage, appearance })?
        .with_name("Mask")
}

#[test]
fn mask_layers_draw_tinted_overlays_and_keep_their_names_and_locks() -> harness::TestResult {
    run(|context| {
        let extent = Extent::new(64, 32)?;
        let mut document = Document::new(1, extent, context.memory())?;
        let coverage = Path::rectangle([0.0, 0.0, 32.0, 32.0])?.rasterize(extent, context)?;
        let layer = mask_layer(1, coverage)?.with_locked(true);
        assert_eq!(layer.name(), "Mask");
        assert!(layer.locked());
        document.apply(DocumentEdit::Insert { index: 0, layer })?;
        let view = Viewport::new(extent)?;
        let compiled = document.snapshot().compile_view(
            view,
            None,
            Pixel::linear(0.0, 0.0, 0.0, 1.0)?,
            Reconstruction::Finished(sugarsubstitute_image_ops::Filter::Nearest),
            context.memory(),
        )?;
        let (graph, output) = compiled.into_parts();
        let mut evaluator = Evaluator::new(graph, NoSource, 16, context.memory())?;
        let tile = evaluator.tile(output, 0, 0, context)?;
        assert_eq!(tile.pixel(5, 5), Some(Pixel::linear(0.5, 0.0, 0.0, 1.0)?));
        assert_eq!(tile.pixel(40, 5), Some(Pixel::linear(0.0, 0.0, 0.0, 1.0)?));
        let wrong = Coverage::new(Extent::new(8, 8)?, 0, context.memory())?;
        assert!(
            document
                .apply(DocumentEdit::Insert { index: 1, layer: mask_layer(2, wrong)? })
                .is_err()
        );
        assert!(MaskAppearance::new([0, 0, 0], 1.5).is_err());
        assert!(
            Layer::new(LayerId::new(3)?, LayerContent::Exposure { stops: 0.0 })?
                .with_name(&"x".repeat(257))
                .is_err()
        );
        Ok(())
    })
}

#[test]
fn selection_edits_are_undoable_and_staged_with_gestures() -> harness::TestResult {
    run(|context| {
        let extent = Extent::new(40, 40)?;
        let mut document = Document::new(1, extent, context.memory())?;
        let selection = Path::ellipse([4.0, 4.0, 30.0, 20.0])?.rasterize(extent, context)?;
        assert!(document.apply(DocumentEdit::Select(Some(selection.clone())))?);
        assert_eq!(document.snapshot().selection(), Some(&selection));
        assert!(!document.apply(DocumentEdit::Select(Some(selection.clone())))?);
        let mut gesture = document.begin_edit(EditSessionId::new(1)?);
        gesture.stage(DocumentEdit::Select(None))?;
        assert_eq!(gesture.preview().selection(), None);
        assert_eq!(document.snapshot().selection(), Some(&selection));
        assert!(document.commit_edit(gesture)?);
        assert_eq!(document.snapshot().selection(), None);
        assert!(document.undo()?);
        assert_eq!(document.snapshot().selection(), Some(&selection));
        let wrong = Coverage::new(Extent::new(8, 8)?, FULL_COVERAGE, context.memory())?;
        assert!(document.apply(DocumentEdit::Select(Some(wrong))).is_err());
        Ok(())
    })
}

#[test]
fn damage_follows_edited_coverage_tiles_and_falls_back_to_the_whole_document() -> harness::TestResult
{
    run(|context| {
        let extent = Extent::new(1000, 600)?;
        let full = Region::new(0, 0, 1000, 600)?;
        let mut document = Document::new(7, extent, context.memory())?;
        let image =
            Layer::new(LayerId::new(1)?, LayerContent::Raster { asset: AssetId([3; 32]), extent })?;
        document.apply(DocumentEdit::Insert { index: 0, layer: image.clone() })?;
        let empty = Coverage::new(extent, 0, context.memory())?;
        document.apply(DocumentEdit::Insert { index: 1, layer: mask_layer(2, empty.clone())? })?;
        let before = document.snapshot();

        // A stroke in one tile damages that tile only.
        let mut painted = empty.clone();
        painted
            .update(Region::new(300, 140, 10, 10)?, context.memory(), |_, _, _| FULL_COVERAGE)?;
        document.apply(DocumentEdit::Replace(mask_layer(2, painted.clone())?))?;
        let stroke = document.snapshot();
        let damage = stroke.damage_since(&before);
        assert_eq!(damage.regions().collect::<Vec<_>>(), [Region::new(256, 128, 128, 128)?]);

        // Recolouring the overlay damages the coverage's bounds; renaming damages nothing.
        let appearance = MaskAppearance::new([0, 0, 255], 0.5)?;
        document.apply(DocumentEdit::Replace(
            mask_layer(2, painted.clone())?
                .with_content(LayerContent::Coverage { coverage: painted.clone(), appearance }),
        ))?;
        let recoloured = document.snapshot();
        assert_eq!(
            recoloured.damage_since(&stroke).regions().collect::<Vec<_>>(),
            [Region::new(300, 140, 10, 10)?]
        );
        document
            .apply(DocumentEdit::Replace(recoloured.layers()[1].clone().with_name("Renamed")?))?;
        assert!(document.snapshot().damage_since(&recoloured).is_empty());

        // The selection is a presentation overlay and never damages the image.
        let renamed = document.snapshot();
        document.apply(DocumentEdit::Select(Some(painted)))?;
        assert!(document.snapshot().damage_since(&renamed).is_empty());

        // Unbounded changes damage everything.
        let selected = document.snapshot();
        document.apply(DocumentEdit::Replace(image.with_visibility(false)))?;
        assert_eq!(
            document.snapshot().damage_since(&selected).regions().collect::<Vec<_>>(),
            [full]
        );
        document.apply(DocumentEdit::Reorder { layer: LayerId::new(2)?, index: 0 })?;
        let reordered = document.snapshot();
        assert_eq!(reordered.damage_since(&selected).regions().collect::<Vec<_>>(), [full]);
        let other = Document::new(8, extent, context.memory())?.snapshot();
        assert_eq!(other.damage_since(&reordered).regions().collect::<Vec<_>>(), [full]);
        assert!(reordered.damage_since(&reordered).is_empty());
        Ok(())
    })
}
