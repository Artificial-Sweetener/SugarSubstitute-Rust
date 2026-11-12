// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose originals into placed document rectangles, as one overview frame does.
use super::*;

const RED_ASSET: AssetId = AssetId([1; 32]);
const BLUE_ASSET: AssetId = AssetId([2; 32]);

/// Two constant-colored originals with different extents.
struct Cells;
impl RasterSource for Cells {
    fn read(
        &mut self,
        asset: AssetId,
        region: Region,
        output: &mut [Pixel],
        _: &JobContext,
    ) -> Result<(), ImageError> {
        let (extent, color) = match asset {
            RED_ASSET => (Extent::new(64, 32)?, Pixel::linear(1.0, 0.0, 0.0, 1.0)?),
            BLUE_ASSET => (Extent::new(16, 48)?, Pixel::linear(0.0, 0.0, 1.0, 1.0)?),
            _ => return Err(ImageError::Invalid("Unknown placed asset")),
        };
        if region.intersection(extent.region()) != Some(region) {
            return Err(ImageError::Invalid("Placed read outside its original"));
        }
        output.fill(color);
        Ok(())
    }
}

fn placed(id: u64, asset: AssetId, extent: Extent, bounds: [f64; 4]) -> Result<Layer, RenderError> {
    Ok(Layer::new(LayerId::new(id)?, LayerContent::PlacedRaster { asset, extent, bounds })?)
}

#[test]
fn placed_originals_fill_only_their_cells_over_the_matte() -> harness::TestResult {
    execute(|context| {
        let surface = Extent::new(80, 40)?;
        let mut document = Document::new(1, surface, context.memory())?;
        let red = placed(1, RED_ASSET, Extent::new(64, 32)?, [0.0, 0.0, 40.0, 20.0])?;
        let blue = placed(2, BLUE_ASSET, Extent::new(16, 48)?, [48.0, 0.0, 10.0, 30.0])?;
        document.apply(DocumentEdit::Insert { index: 0, layer: red })?;
        document.apply(DocumentEdit::Insert { index: 1, layer: blue })?;
        let snapshot = document.snapshot();
        let view = Viewport::new(surface)?;
        let mut renderer = CpuRenderer::new(Cells, 64, context.memory())?;
        let update = renderer.prepare(
            RenderRequest {
                document: &snapshot,
                comparison: None,
                view,
                reconstruction: Reconstruction::Finished(Filter::Lanczos3),
                matte: black()?,
                target: stamp(1)?,
                base: None,
                damage: DamagePlan::full(view.surface()),
            },
            context,
        )?;
        let mut target = CpuSurface::new(stamp(1)?.surface, surface, context.memory())?;
        target.apply(&update)?;
        for (index, &pixel) in target.pixels().iter().enumerate() {
            let (x, y) = (index as u32 % surface.width(), index as u32 / surface.width());
            let expected = if x < 40 && y < 20 {
                0xffff_0000
            } else if (48..58).contains(&x) && y < 30 {
                0xff00_00ff
            } else {
                0xff00_0000
            };
            assert_eq!(pixel, expected, "pixel ({x}, {y})");
        }
        Ok(())
    })
}

#[test]
fn placed_raster_bounds_must_be_finite_and_nonempty() -> harness::TestResult {
    execute(|context| {
        let extent = Extent::new(16, 48)?;
        for bounds in [[0.0, 0.0, 0.0, 4.0], [0.0, 0.0, 4.0, -1.0], [f64::NAN, 0.0, 4.0, 4.0]] {
            let layer = placed(1, BLUE_ASSET, extent, bounds)?;
            let mut document = Document::new(1, Extent::new(8, 8)?, context.memory())?;
            assert!(document.apply(DocumentEdit::Insert { index: 0, layer }).is_err());
        }
        Ok(())
    })
}
