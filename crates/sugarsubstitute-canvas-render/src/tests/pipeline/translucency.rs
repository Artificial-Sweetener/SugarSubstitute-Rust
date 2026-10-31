// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Minified nearest and Lanczos reconstruction share one canonical translucent composite.

use super::*;

#[test]
fn minified_nearest_and_lanczos_share_canonical_translucent_composite() -> harness::TestResult {
    execute(|context| {
        let extent = Extent::new(64, 64)?;
        let mut document = Document::new(1, extent, context.memory())?;
        let layer =
            Layer::new(LayerId::new(1)?, LayerContent::Raster { asset: AssetId([1; 32]), extent })?
                .with_opacity(0.75)?
                .with_mask(Some(LayerMask::Raster { asset: AssetId([1; 32]), extent }));
        document.apply(DocumentEdit::Insert { index: 0, layer })?;
        let snapshot = document.snapshot();
        let mut view = Viewport::new(Extent::new(8, 8)?)?;
        view.zoom_at(0.125, [0.0; 2])?;
        let source = Pixel::linear(0.8, 0.2, 0.4, 0.5)?;
        let mut nearest_renderer =
            CpuRenderer::new(Gradient { extent, constant: Some(source) }, 64, context.memory())?;
        let nearest = CpuSurface::from_prepared(nearest_renderer.prepare(
            RenderRequest {
                document: &snapshot,
                comparison: None,
                view,
                reconstruction: Reconstruction::Finished(Filter::Nearest),
                matte: black()?,
                target: stamp(1)?,
                base: None,
                damage: DamagePlan::full(view.surface()),
            },
            context,
        )?)?;
        let mut lanczos_renderer =
            CpuRenderer::new(Gradient { extent, constant: Some(source) }, 64, context.memory())?;
        let lanczos = CpuSurface::from_prepared(lanczos_renderer.prepare(
            RenderRequest {
                document: &snapshot,
                comparison: None,
                view,
                reconstruction: Reconstruction::Finished(Filter::Lanczos3),
                matte: black()?,
                target: stamp(2)?,
                base: None,
                damage: DamagePlan::full(view.surface()),
            },
            context,
        )?)?;
        assert_eq!(nearest.pixels(), lanczos.pixels());
        let expected =
            SrgbEncoder::new(context.memory())?.encode(Pixel::linear(0.15, 0.0375, 0.075, 1.0)?)?;
        let expected = u32::from_be_bytes([255, expected[0], expected[1], expected[2]]);
        assert!(nearest.pixels().iter().all(|pixel| *pixel == expected));
        Ok(())
    })
}
