// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Render real document DAG content through exact viewport repair and canonical display encoding.
use super::*;
mod coverage;
mod phase;
mod progressive;
mod reconstruction;
use sugarsubstitute_canvas::{
    Document, DocumentEdit, Layer, LayerContent, LayerId, LayerMask, Viewport,
};
use sugarsubstitute_image_ops::{
    AssetId, Filter, Pixel, RasterSource, Reconstruction, SrgbEncoder, VectorRectangle,
};

struct Gradient {
    extent: Extent,
    constant: Option<Pixel>,
}
impl RasterSource for Gradient {
    fn read(
        &mut self,
        asset: AssetId,
        region: Region,
        output: &mut [Pixel],
        _: &JobContext,
    ) -> Result<(), ImageError> {
        if asset != AssetId([1; 32]) || region.intersection(self.extent.region()) != Some(region) {
            return Err(ImageError::Invalid("Invalid pipeline source request"));
        }
        for y in region.y()..region.bottom() {
            for x in region.x()..region.right() {
                output[(y - region.y()) as usize * region.width() as usize
                    + (x - region.x()) as usize] = match self.constant {
                    Some(pixel) => pixel,
                    None => {
                        Pixel::linear((x % 251) as f32 / 250.0, (y % 239) as f32 / 238.0, 0.3, 0.5)?
                    }
                };
            }
        }
        Ok(())
    }
}
fn image_document(extent: Extent, context: &JobContext) -> Result<Document, RenderError> {
    let mut document = Document::new(1, extent, context.memory())?;
    let layer =
        Layer::new(LayerId::new(1)?, LayerContent::Raster { asset: AssetId([1; 32]), extent })?;
    document.apply(DocumentEdit::Insert { index: 0, layer })?;
    Ok(document)
}
fn black() -> Result<Pixel, ImageError> {
    Pixel::linear(0.0, 0.0, 0.0, 1.0)
}

#[test]
fn repeated_tall_pan_repairs_retain_warmed_source_chunks_under_view_cache_churn()
-> harness::TestResult {
    execute(|context| {
        let extent = Extent::new(4096, 4096)?;
        let snapshot = image_document(extent, context)?.snapshot();
        let mut view = Viewport::new(Extent::new(512, 2048)?)?;
        view.pan_by([-1024.0, -1024.0])?;
        let mut renderer =
            CpuRenderer::new(Gradient { extent, constant: None }, 64, context.memory())?;
        let first = renderer.prepare(
            RenderRequest {
                reconstruction: Reconstruction::Finished(Filter::Lanczos3),
                document: &snapshot,
                view,
                matte: black()?,
                target: stamp(1)?,
                base: None,
                damage: DamagePlan::full(view.surface()),
            },
            context,
        )?;
        drop(first);
        let previous = view;
        view.pan_by([-5.0, 0.0])?;
        let repaired = renderer.prepare(
            RenderRequest {
                reconstruction: Reconstruction::Finished(Filter::Lanczos3),
                document: &snapshot,
                view,
                matte: black()?,
                target: stamp(2)?,
                base: Some(stamp(1)?),
                damage: view.repair_from(previous)?,
            },
            context,
        )?;
        drop(repaired);
        let warmed = renderer.stats().source_pixels;
        let previous = view;
        view.pan_by([-5.0, 0.0])?;
        let repaired = renderer.prepare(
            RenderRequest {
                reconstruction: Reconstruction::Finished(Filter::Lanczos3),
                document: &snapshot,
                view,
                matte: black()?,
                target: stamp(3)?,
                base: Some(stamp(2)?),
                damage: view.repair_from(previous)?,
            },
            context,
        )?;
        drop(repaired);
        assert_eq!(
            renderer.stats().source_pixels,
            warmed,
            "Cheap viewport intermediates must not evict warmed source tiles during edge repair"
        );
        Ok(())
    })
}
fn execute(
    action: impl FnOnce(&JobContext) -> Result<(), RenderError> + Send + 'static,
) -> harness::TestResult {
    harness::run(64 * 1024 * 1024, |context| {
        action(context).map_err(|cause| ImageError::Source(Box::new(cause)))
    })
}

#[test]
fn dag_pan_repair_matches_full_redraw_and_reduces_actual_image_work() -> harness::TestResult {
    execute(|context| {
        let extent = Extent::new(513, 387)?;
        let document = image_document(extent, context)?;
        let snapshot = document.snapshot();
        let mut view = Viewport::new(Extent::new(320, 200)?)?;
        let mut renderer =
            CpuRenderer::new(Gradient { extent, constant: None }, 64, context.memory())?;
        let first = renderer.prepare(
            RenderRequest {
                reconstruction: Reconstruction::Finished(Filter::Lanczos3),
                document: &snapshot,
                view,
                matte: black()?,
                target: stamp(1)?,
                base: None,
                damage: DamagePlan::full(view.surface()),
            },
            context,
        )?;
        let mut surface = CpuSurface::new(stamp(1)?.surface, view.surface(), context.memory())?;
        surface.apply(&first)?;
        drop(first);
        let previous = view;
        view.pan_by([5.0, -3.0])?;
        let before = renderer.stats();
        let damage = view.repair_from(previous)?;
        let update = renderer.prepare(
            RenderRequest {
                reconstruction: Reconstruction::Finished(Filter::Lanczos3),
                document: &snapshot,
                view,
                matte: black()?,
                target: stamp(2)?,
                base: Some(stamp(1)?),
                damage,
            },
            context,
        )?;
        let work = surface.apply(&update)?;
        assert_eq!(work.repaired_pixels, 1945);
        assert_eq!(work.reused_pixels, 62055);
        let repair_pixels = renderer.stats().computed_pixels - before.computed_pixels;
        let mut reference =
            CpuRenderer::new(Gradient { extent, constant: None }, 64, context.memory())?;
        let full = reference.prepare(
            RenderRequest {
                reconstruction: Reconstruction::Finished(Filter::Lanczos3),
                document: &snapshot,
                view,
                matte: black()?,
                target: stamp(3)?,
                base: None,
                damage: DamagePlan::full(view.surface()),
            },
            context,
        )?;
        let mut full_surface =
            CpuSurface::new(stamp(1)?.surface, view.surface(), context.memory())?;
        full_surface.apply(&full)?;
        assert_eq!(surface.pixels(), full_surface.pixels());
        assert!(
            repair_pixels * 2 < reference.stats().computed_pixels,
            "Repair computed {repair_pixels} pixels versus {}",
            reference.stats().computed_pixels
        );
        Ok(())
    })
}

#[test]
fn vector_layers_rasterize_at_requested_zoom_without_flattening_geometry() -> harness::TestResult {
    execute(|context| {
        let extent = Extent::new(64, 64)?;
        let mut document = Document::new(1, extent, context.memory())?;
        let shape =
            VectorRectangle::new([1.25, 1.25, 2.25, 2.25], Pixel::linear(1.0, 0.0, 0.0, 1.0)?)?;
        let layer = Layer::new(LayerId::new(1)?, LayerContent::VectorRectangle(shape))?;
        document.apply(DocumentEdit::Insert { index: 0, layer })?;
        let snapshot = document.snapshot();
        let mut view = Viewport::new(Extent::new(32, 32)?)?;
        view.zoom_at(8.0, [0.0; 2])?;
        let mut renderer =
            CpuRenderer::new(Gradient { extent, constant: None }, 32, context.memory())?;
        let frame = renderer.prepare(
            RenderRequest {
                reconstruction: Reconstruction::Finished(Filter::Lanczos3),
                document: &snapshot,
                view,
                matte: black()?,
                target: stamp(1)?,
                base: None,
                damage: DamagePlan::full(view.surface()),
            },
            context,
        )?;
        let mut surface = CpuSurface::new(stamp(1)?.surface, view.surface(), context.memory())?;
        surface.apply(&frame)?;
        assert_eq!(surface.pixels()[10 * 32 + 9], 0xff000000);
        assert_eq!(surface.pixels()[10 * 32 + 10], 0xffff0000);
        assert_eq!(surface.pixels()[17 * 32 + 17], 0xffff0000);
        assert_eq!(surface.pixels()[18 * 32 + 18], 0xff000000);
        assert_eq!(snapshot.layers()[0].content(), LayerContent::VectorRectangle(shape));
        assert_eq!(renderer.stats().source_pixels, 0);
        Ok(())
    })
}

#[test]
fn adjustment_layer_mask_and_opacity_preserve_original_translucent_coverage() -> harness::TestResult
{
    execute(|context| {
        let extent = Extent::new(4, 4)?;
        let mut document = image_document(extent, context)?;
        let mask = VectorRectangle::new([0.0, 0.0, 2.0, 4.0], Pixel::linear(0.0, 0.0, 0.0, 1.0)?)?;
        let adjustment = Layer::new(LayerId::new(2)?, LayerContent::Exposure { stops: 1.0 })?
            .with_opacity(0.5)?
            .with_mask(Some(LayerMask::VectorRectangle(mask)));
        document.apply(DocumentEdit::Insert { index: 1, layer: adjustment })?;
        let snapshot = document.snapshot();
        let view = Viewport::new(extent)?;
        let original = Pixel::linear(0.4, 0.2, 0.1, 0.25)?;
        let mut renderer =
            CpuRenderer::new(Gradient { extent, constant: Some(original) }, 32, context.memory())?;
        let frame = renderer.prepare(
            RenderRequest {
                reconstruction: Reconstruction::Finished(Filter::Lanczos3),
                document: &snapshot,
                view,
                matte: black()?,
                target: stamp(1)?,
                base: None,
                damage: DamagePlan::full(extent),
            },
            context,
        )?;
        let mut surface = CpuSurface::new(stamp(1)?.surface, extent, context.memory())?;
        surface.apply(&frame)?;
        let encoder = SrgbEncoder::new(context.memory())?;
        let word = |pixel| -> Result<u32, RenderError> {
            let [r, g, b, _] = encoder.encode(pixel)?;
            Ok(u32::from_be_bytes([255, r, g, b]))
        };
        assert_eq!(surface.pixels()[0], word(Pixel::linear(0.15, 0.075, 0.0375, 1.0)?)?);
        assert_eq!(surface.pixels()[3], word(Pixel::linear(0.1, 0.05, 0.025, 1.0)?)?);
        Ok(())
    })
}

#[test]
fn completely_offscreen_huge_images_do_not_trigger_decode_or_pyramid_work() -> harness::TestResult {
    execute(|context| {
        let extent = Extent::new(1_000_000_000, 1_000_000_000)?;
        let document = image_document(extent, context)?;
        let snapshot = document.snapshot();
        let mut view = Viewport::new(Extent::new(64, 64)?)?;
        view.pan_by([1e50, 1e50])?;
        let mut renderer =
            CpuRenderer::new(Gradient { extent, constant: None }, 8, context.memory())?;
        let frame = renderer.prepare(
            RenderRequest {
                reconstruction: Reconstruction::Finished(Filter::Lanczos3),
                document: &snapshot,
                view,
                matte: black()?,
                target: stamp(1)?,
                base: None,
                damage: DamagePlan::full(view.surface()),
            },
            context,
        )?;
        let mut surface = CpuSurface::new(stamp(1)?.surface, view.surface(), context.memory())?;
        surface.apply(&frame)?;
        assert!(surface.pixels().iter().all(|pixel| *pixel == 0xff000000));
        assert_eq!(renderer.stats().source_pixels, 0);
        assert_eq!(renderer.stats().computed_tiles, 1);
        Ok(())
    })
}

#[test]
fn minified_one_pixel_axes_preserve_geometric_image_coverage() -> harness::TestResult {
    execute(|context| {
        let extent = Extent::new(1, 100)?;
        let document = image_document(extent, context)?;
        let snapshot = document.snapshot();
        let mut view = Viewport::new(Extent::new(16, 16)?)?;
        view.zoom_at(0.1, [0.0; 2])?;
        let mut renderer = CpuRenderer::new(
            Gradient { extent, constant: Some(Pixel::linear(1.0, 0.0, 0.0, 1.0)?) },
            32,
            context.memory(),
        )?;
        let frame = renderer.prepare(
            RenderRequest {
                reconstruction: Reconstruction::Finished(Filter::Lanczos3),
                document: &snapshot,
                view,
                matte: black()?,
                target: stamp(1)?,
                base: None,
                damage: DamagePlan::full(view.surface()),
            },
            context,
        )?;
        let mut surface = CpuSurface::new(stamp(1)?.surface, view.surface(), context.memory())?;
        surface.apply(&frame)?;
        let encoder = SrgbEncoder::new(context.memory())?;
        let [red, green, blue, _] = encoder.encode(Pixel::linear(0.1, 0.0, 0.0, 1.0)?)?;
        assert_eq!(surface.pixels()[4 * 16], u32::from_be_bytes([255, red, green, blue]));
        assert_eq!(surface.pixels()[4 * 16 + 1], 0xff000000);
        assert_eq!(surface.pixels()[11 * 16], 0xff000000);
        Ok(())
    })
}
