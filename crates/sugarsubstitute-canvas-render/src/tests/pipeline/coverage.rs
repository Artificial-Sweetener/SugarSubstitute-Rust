// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove padded canonical pixels crop to the independently compiled target composition.

use super::*;
use sugarsubstitute_canvas::BackingCoverage;
use sugarsubstitute_image_ops::Reconstruction;

#[test]
fn padded_crop_matches_direct_mixed_composition_at_finished_zoom_boundaries() -> harness::TestResult
{
    execute(|context| {
        let extent = Extent::new(257, 193)?;
        let mut document = Document::new(1, extent, context.memory())?;
        let raster =
            Layer::new(LayerId::new(1)?, LayerContent::Raster { asset: AssetId([1; 32]), extent })?
                .with_opacity(0.78)?
                .with_mask(Some(half_mask(extent, context)?));
        document.apply(DocumentEdit::Insert { index: 0, layer: raster })?;
        let vector = VectorRectangle::new(
            [24.125, 18.375, 96.875, 79.625],
            Pixel::linear(0.1, 0.8, 0.25, 0.65)?,
        )?;
        let vector = Layer::new(LayerId::new(2)?, LayerContent::VectorRectangle(vector))?
            .with_opacity(0.61)?;
        document.apply(DocumentEdit::Insert { index: 1, layer: vector })?;
        let adjustment_mask = soft_rectangle(extent, [29.75, 21.125, 83.5, 68.875], 0.72, context)?;
        let adjustment = Layer::new(LayerId::new(3)?, LayerContent::Exposure { stops: 0.8 })?
            .with_opacity(0.57)?
            .with_mask(Some(adjustment_mask));
        document.apply(DocumentEdit::Insert { index: 2, layer: adjustment })?;
        let snapshot = document.snapshot();
        let target_extent = Extent::new(39, 27)?;

        for (zoom, generation) in [(0.37, 1_u64), (1.37, 4), (2.0, 7), (3.5, 10)] {
            let mut target = Viewport::new(target_extent)?;
            target.reframe([31.375, 22.625], zoom)?;
            target.pan_by([7.0, -5.0])?;
            let coverage = BackingCoverage::new(target, [17, 13])?;
            assert_eq!(coverage.margin(), [17, 13]);
            let crop = coverage
                .crop(target, extent)?
                .ok_or(RenderError::Invalid("Construction target lost padded coverage"))?;
            let reconstruction = Reconstruction::finished_for_display_zoom(zoom)?;

            let mut padded_renderer =
                CpuRenderer::new(Gradient { extent, constant: None }, 64, context.memory())?;
            let padded = CpuSurface::from_prepared(padded_renderer.prepare(
                RenderRequest {
                    document: &snapshot,
                    comparison: None,
                    view: coverage.viewport(),
                    reconstruction,
                    matte: black()?,
                    target: stamp(generation)?,
                    base: None,
                    damage: DamagePlan::full(coverage.viewport().surface()),
                },
                context,
            )?)?;
            let projected = ProjectionBuffer::new(target_extent, context.memory(), context)?
                .project(stamp(generation + 1)?, &padded, crop, black()?, context)?;

            let mut direct_renderer =
                CpuRenderer::new(Gradient { extent, constant: None }, 64, context.memory())?;
            let direct = CpuSurface::from_prepared(direct_renderer.prepare(
                RenderRequest {
                    document: &snapshot,
                    comparison: None,
                    view: target,
                    reconstruction,
                    matte: black()?,
                    target: stamp(generation + 2)?,
                    base: None,
                    damage: DamagePlan::full(target_extent),
                },
                context,
            )?)?;
            assert_eq!(projected.pixels(), direct.pixels(), "zoom={zoom}");
        }
        Ok(())
    })
}
