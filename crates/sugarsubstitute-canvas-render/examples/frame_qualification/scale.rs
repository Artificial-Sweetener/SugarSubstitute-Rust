// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise full 4K GPU replacements and pan copies under the unchanged admitted memory limit.
use std::{error::Error, sync::Arc, time::Instant};
use sugarsubstitute_canvas::*;
use sugarsubstitute_canvas_render::{GpuRenderer, GpuSurface};
use sugarsubstitute_contracts::execution::{Generation, JobContext, SurfaceId};
use sugarsubstitute_image_gpu::GpuDevice;
use sugarsubstitute_image_ops::*;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;
struct UnusedSource;
impl RasterSource for UnusedSource {
    fn read(
        &mut self,
        _: AssetId,
        _: Region,
        _: &mut [Pixel],
        _: &JobContext,
    ) -> std::result::Result<(), ImageError> {
        Err(ImageError::Invalid("Vector-only qualification requested a raster source"))
    }
}
fn shape(extent: Extent, changed: bool) -> Result<Layer> {
    let color = if changed { [0.75, 0.25, 0.5] } else { [0.25, 0.5, 0.75] };
    Ok(Layer::new(
        LayerId::new(1)?,
        LayerContent::VectorRectangle(VectorRectangle::new(
            [0.0, 0.0, f64::from(extent.width()), f64::from(extent.height())],
            Pixel::linear(color[0], color[1], color[2], 1.0)?,
        )?),
    )?)
}
pub(super) fn qualify(device: Arc<GpuDevice>, context: &JobContext) -> Result<usize> {
    let extent = Extent::new(3840, 2160)?;
    let identity = SurfaceId::new(2)?;
    let mut document = Document::new(2, extent, context.memory())?;
    document.apply(DocumentEdit::Insert { index: 0, layer: shape(extent, false)? })?;
    let mut view = Viewport::new(extent)?;
    view.fit(extent)?;
    let mut renderer =
        GpuRenderer::new(UnusedSource, 256, device.clone(), context.memory(), context)?;
    let mut retained = GpuSurface::new(identity, extent, device)?;
    let encoder = SrgbEncoder::new(context.memory())?;
    let matte = Pixel::linear(0.0625, 0.125, 0.25, 1.0)?;
    for phase in 0..3_u64 {
        let previous = view;
        if phase == 1 {
            view.pan_by([7.0, -11.0])?;
        }
        if phase == 2 {
            document.apply(DocumentEdit::Replace(shape(extent, true)?))?;
        }
        let snapshot = document.snapshot();
        let damage =
            if phase == 1 { view.repair_from(previous)? } else { DamagePlan::full(extent) };
        let base = retained.current();
        let target = FrameStamp { surface: identity, generation: Generation::new(phase + 1)? };
        let started = Instant::now();
        let prepared = renderer.prepare(
            RenderRequest {
                document: &snapshot,
                view,
                reconstruction: Reconstruction::Finished(Filter::Lanczos3),
                matte,
                target,
                base,
                damage,
            },
            context,
        )?;
        let prepare_us = started.elapsed().as_micros();
        let started = Instant::now();
        let before_tiles = renderer.stats().resident_tiles;
        let work = renderer.retain(&mut retained, &prepared, context)?;
        let released_tiles = before_tiles.saturating_sub(renderer.stats().resident_tiles);
        let copy_us = started.elapsed().as_micros();
        drop(prepared);
        let frame = retained.frame().ok_or("4K retained frame is unavailable")?;
        let (graph, output) = snapshot
            .compile_view(
                view,
                matte,
                Reconstruction::Finished(Filter::Lanczos3),
                context.memory(),
            )?
            .into_parts();
        let mut cpu = Evaluator::new(graph, UnusedSource, 32, context.memory())?;
        for region in extent.region().tiles() {
            let actual = frame.image().read_region(region, context)?;
            let expected = cpu.region(output, region, context)?;
            for y in region.y()..region.bottom() {
                for x in region.x()..region.right() {
                    let [r, g, b, _] = encoder
                        .encode(expected.pixel(x, y).ok_or("Missing 4K reference pixel")?)?;
                    let word = u32::from_be_bytes([255, r, g, b]);
                    let index = ((y - region.y()) * region.width() + x - region.x()) as usize;
                    if actual.pixels()[index] != word {
                        return Err("4K GPU retained image differs from CPU DAG".into());
                    }
                }
            }
        }
        println!(
            "CANVAS_4K: phase={phase} pixels={} prepare_us={prepare_us} copy_us={copy_us} repaired_pixels={} reused_pixels={} released_tiles={released_tiles} exact_cpu_agreement=true",
            extent.pixels(),
            work.repaired_pixels,
            work.reused_pixels
        );
    }
    Ok(3)
}
