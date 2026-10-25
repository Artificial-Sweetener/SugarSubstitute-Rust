// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compare repaired pixels with full graph reconstruction after exact integer moves at fractional zoom.
use super::*;
use sugarsubstitute_image_ops::Filter;

#[test]
fn fractional_zoom_repairs_match_full_raster_vector_mask_and_adjustment_composites()
-> harness::TestResult {
    execute(|context| {
        let extent = Extent::new(513, 387)?;
        let mut document = Document::new(1, extent, context.memory())?;
        let raster =
            Layer::new(LayerId::new(1)?, LayerContent::Raster { asset: AssetId([1; 32]), extent })?
                .with_mask(Some(LayerMask::Raster { asset: AssetId([1; 32]), extent }));
        document.apply(DocumentEdit::Insert { index: 0, layer: raster })?;
        let vector = VectorRectangle::new(
            [-3.25, 7.125, 402.75, 364.25],
            Pixel::linear(2.0, -0.5, 0.3, 0.7)?,
        )?;
        document.apply(DocumentEdit::Insert {
            index: 1,
            layer: Layer::new(LayerId::new(2)?, LayerContent::VectorRectangle(vector))?,
        })?;
        let mask =
            VectorRectangle::new([2.25, 1.125, 470.75, 350.5], Pixel::linear(0.0, 0.0, 0.0, 1.0)?)?;
        let adjustment = Layer::new(LayerId::new(3)?, LayerContent::Exposure { stops: 0.75 })?
            .with_opacity(0.55)?
            .with_mask(Some(LayerMask::VectorRectangle(mask)));
        document.apply(DocumentEdit::Insert { index: 2, layer: adjustment })?;
        let snapshot = document.snapshot();
        let mut generation = 0;
        for filter in [Filter::Nearest, Filter::Lanczos3] {
            for zoom in [0.37, 1.37] {
                let mut view = Viewport::new(Extent::new(333, 209)?)?;
                view.zoom_at(zoom, [167.25, 83.75])?;
                let mut renderer =
                    CpuRenderer::new(Gradient { extent, constant: None }, 64, context.memory())?;
                generation += 1;
                let mut surface = CpuSurface::from_prepared(renderer.prepare(
                    RenderRequest {
                        document: &snapshot,
                        view,
                        filter,
                        matte: black()?,
                        target: stamp(generation)?,
                        base: None,
                        damage: DamagePlan::full(view.surface()),
                    },
                    context,
                )?)?;
                for delta in [[13.0, -7.0], [-5.0, 3.0], [3.0, 19.0], [-17.0, -15.0], [6.0, 0.0]] {
                    let previous = view;
                    view.pan_by(delta)?;
                    let damage = view.repair_from(previous)?;
                    assert!(damage.blit().is_some());
                    assert!(damage.dirty().pixels() < view.surface().pixels());
                    generation += 1;
                    let target = stamp(generation)?;
                    let repair = renderer.prepare(
                        RenderRequest {
                            document: &snapshot,
                            view,
                            filter,
                            matte: black()?,
                            target,
                            base: surface.current(),
                            damage,
                        },
                        context,
                    )?;
                    surface.apply(&repair)?;
                    let full = CpuSurface::from_prepared(renderer.prepare(
                        RenderRequest {
                            document: &snapshot,
                            view,
                            filter,
                            matte: black()?,
                            target,
                            base: None,
                            damage: DamagePlan::full(view.surface()),
                        },
                        context,
                    )?)?;
                    assert_eq!(
                        surface.pixels(),
                        full.pixels(),
                        "zoom={zoom} filter={filter:?} delta={delta:?}"
                    );
                }
            }
        }
        Ok(())
    })
}

struct AlternatingSource;
impl RasterSource for AlternatingSource {
    fn read(
        &mut self,
        _: AssetId,
        region: Region,
        output: &mut [Pixel],
        _: &JobContext,
    ) -> Result<(), ImageError> {
        for y in 0..region.height() {
            for x in 0..region.width() {
                output[(y * region.width() + x) as usize] = if (region.x() + x).is_multiple_of(2) {
                    Pixel::linear(1.0, 0.0, 0.0, 1.0)?
                } else {
                    Pixel::linear(0.0, 0.0, 1.0, 1.0)?
                };
            }
        }
        Ok(())
    }
}

#[test]
fn nearest_pan_preserves_the_source_pixel_where_offset_rounding_would_choose_its_neighbor()
-> harness::TestResult {
    execute(|context| {
        let extent = Extent::new(65536, 16384)?;
        let snapshot = image_document(extent, context)?.snapshot();
        let mut view = Viewport::new(Extent::new(1920, 128)?)?;
        view.zoom_at(800.0 / 16384.0, [0.0; 2])?;
        let mut renderer = CpuRenderer::new(AlternatingSource, 64, context.memory())?;
        let mut surface = CpuSurface::from_prepared(renderer.prepare(
            RenderRequest {
                document: &snapshot,
                view,
                filter: Filter::Nearest,
                matte: black()?,
                target: stamp(1)?,
                base: None,
                damage: DamagePlan::full(view.surface()),
            },
            context,
        )?)?;
        assert_eq!(surface.pixels()[1920 + 1587], 0xffff0000);
        let before = view;
        view.pan_by([13.0, 0.0])?;
        let damage = view.repair_from(before)?;
        assert!(damage.blit().is_some());
        surface.apply(&renderer.prepare(
            RenderRequest {
                document: &snapshot,
                view,
                filter: Filter::Nearest,
                matte: black()?,
                target: stamp(2)?,
                base: Some(stamp(1)?),
                damage,
            },
            context,
        )?)?;
        let full = CpuSurface::from_prepared(renderer.prepare(
            RenderRequest {
                document: &snapshot,
                view,
                filter: Filter::Nearest,
                matte: black()?,
                target: stamp(2)?,
                base: None,
                damage: DamagePlan::full(view.surface()),
            },
            context,
        )?)?;
        assert_eq!(surface.pixels()[1920 + 1600], 0xffff0000);
        assert_eq!(surface.pixels(), full.pixels());
        Ok(())
    })
}
