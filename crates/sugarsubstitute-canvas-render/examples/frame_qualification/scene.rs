// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compare GPU document preparation and exact repair readback with complete CPU/GPU references.
use std::{error::Error, sync::Arc, time::Instant};
use sugarsubstitute_canvas::*;
use sugarsubstitute_canvas_render::{CpuSurface, GpuRenderer, PreparedGpuUpdate, PreparedUpdate};
use sugarsubstitute_contracts::execution::{Generation, JobContext, SurfaceId};
use sugarsubstitute_image_gpu::GpuDevice;
use sugarsubstitute_image_ops::*;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;
const ASSET: AssetId = AssetId([19; 32]);
struct Gradient(Extent);
impl RasterSource for Gradient {
    fn read(
        &mut self,
        asset: AssetId,
        region: Region,
        output: &mut [Pixel],
        _: &JobContext,
    ) -> std::result::Result<(), ImageError> {
        if asset != ASSET || region.intersection(self.0.region()) != Some(region) {
            return Err(ImageError::Invalid("Invalid frame qualification source"));
        }
        for y in region.y()..region.bottom() {
            for x in region.x()..region.right() {
                output[((y - region.y()) * region.width() + x - region.x()) as usize] =
                    Pixel::linear((x % 251) as f32 / 250.0, (y % 239) as f32 / 238.0, 0.3, 0.5)?;
            }
        }
        Ok(())
    }
}
fn stamp(value: u64) -> Result<FrameStamp> {
    Ok(FrameStamp { surface: SurfaceId::new(1)?, generation: Generation::new(value)? })
}
fn document(extent: Extent, context: &JobContext) -> Result<Document> {
    let mut document = Document::new(1, extent, context.memory())?;
    let raster = Layer::new(LayerId::new(1)?, LayerContent::Raster { asset: ASSET, extent })?;
    document.apply(DocumentEdit::Insert { index: 0, layer: raster })?;
    let shape = VectorRectangle::new(
        [126.375, 91.25, 900.75, 731.125],
        Pixel::linear(1.4, -0.2, 0.3, 0.625)?,
    )?;
    document.apply(DocumentEdit::Insert {
        index: 1,
        layer: Layer::new(LayerId::new(2)?, LayerContent::VectorRectangle(shape))?,
    })?;
    let mask =
        VectorRectangle::new([199.125, 0.0, 701.75, 769.0], Pixel::linear(0.0, 0.0, 0.0, 1.0)?)?;
    let adjustment = Layer::new(LayerId::new(3)?, LayerContent::Exposure { stops: 0.75 })?
        .with_opacity(0.375)?
        .with_mask(Some(LayerMask::VectorRectangle(mask)));
    document.apply(DocumentEdit::Insert { index: 2, layer: adjustment })?;
    Ok(document)
}
fn readback(
    prepared: &PreparedGpuUpdate,
    reference: &mut Evaluator<Gradient>,
    output: NodeId,
    encoder: &SrgbEncoder,
    context: &JobContext,
) -> Result<(PreparedUpdate, f32)> {
    let mut worst = 0.0_f32;
    let update = PreparedUpdate::prepare(
        prepared.target(),
        prepared.base(),
        *prepared.plan(),
        context.memory(),
        |region, pixels| {
            for tile in prepared.tiles() {
                let Some(overlap) = tile.region().intersection(region) else { continue };
                let actual = tile.read(context)?;
                let expected = reference.region(output, overlap, context)?;
                for y in overlap.y()..overlap.bottom() {
                    for x in overlap.x()..overlap.right() {
                        let from = ((y - tile.region().y()) * tile.region().width() + x
                            - tile.region().x()) as usize;
                        let pixel = actual.pixels()[from];
                        let expected = expected
                            .pixel(x, y)
                            .ok_or(ImageError::Invalid("Missing reference pixel"))?;
                        let [r, g, b, _] = encoder.encode(expected)?;
                        let expected = u32::from_be_bytes([255, r, g, b]);
                        for shift in [0, 8, 16] {
                            let difference = (((pixel >> shift) & 255) as i32
                                - ((expected >> shift) & 255) as i32)
                                .unsigned_abs();
                            worst = worst.max(difference as f32);
                            if difference > 1 {
                                return Err(ImageError::Invalid(
                                    "GPU display differs from CPU reference",
                                )
                                .into());
                            }
                        }
                        pixels[((y - region.y()) * region.width() + x - region.x()) as usize] =
                            pixel;
                    }
                }
            }
            Ok(())
        },
    )?;
    Ok((update, worst))
}
pub(super) fn qualify(device: Arc<GpuDevice>, context: &JobContext) -> Result<usize> {
    let image = Extent::new(2048, 1536)?;
    let extent = Extent::new(1024, 768)?;
    let mut document = document(image, context)?;
    let mut view = Viewport::new(extent)?;
    view.fit(image)?;
    let mut gpu = GpuRenderer::new(Gradient(image), 256, device, context.memory(), context)?;
    let mut surface = CpuSurface::new(stamp(1)?.surface, extent, context.memory())?;
    let encoder = SrgbEncoder::new(context.memory())?;
    let matte = Pixel::linear(0.12, 0.13, 0.14, 1.0)?;
    for phase in 0..7_u64 {
        let previous = view;
        let filter = if phase == 2 { Filter::Nearest } else { Filter::Lanczos3 };
        match phase {
            1 => view.pan_by([7.0, -11.0])?,
            2 => view.zoom_at(1.3, [187.5, 153.5])?,
            4 => view.pan_by([-13.0, 5.0])?,
            5 => {
                if !document.undo()? {
                    return Err("Missing document undo".into());
                }
            }
            6 => {
                if !document.redo()? {
                    return Err("Missing document redo".into());
                }
            }
            _ => {}
        }
        let snapshot = document.snapshot();
        let damage = if phase == 1 || phase == 4 {
            view.repair_from(previous)?
        } else {
            DamagePlan::full(extent)
        };
        let base = if phase == 0 { None } else { Some(stamp(phase)?) };
        let before = gpu.work();
        let started = Instant::now();
        let prepared = gpu.prepare(
            RenderRequest {
                document: &snapshot,
                view,
                filter,
                matte,
                target: stamp(phase + 1)?,
                base,
                damage,
            },
            context,
        )?;
        let prepare_us = started.elapsed().as_micros();
        let prepared_work = gpu.work();
        let prepared_bytes: u64 =
            prepared.tiles().iter().map(|tile| tile.region().pixels() * 4).sum();
        if prepared.target() != stamp(phase + 1)?
            || prepared.base() != base
            || prepared.plan() != &damage
        {
            return Err("GPU frame changed publication metadata".into());
        }
        let compiled = snapshot.compile_view(view, matte, filter, context.memory())?;
        let (graph, output) = compiled.into_parts();
        let mut cpu = Evaluator::new(graph, Gradient(image), 32, context.memory())?;
        let (update, worst) = readback(&prepared, &mut cpu, output, &encoder, context)?;
        let repair = surface.apply(&update)?;
        drop(update);
        drop(prepared);
        let full = gpu.prepare(
            RenderRequest {
                document: &snapshot,
                view,
                filter,
                matte,
                target: stamp(100 + phase)?,
                base: None,
                damage: DamagePlan::full(extent),
            },
            context,
        )?;
        let (full_update, _) = readback(&full, &mut cpu, output, &encoder, context)?;
        let mut full_surface = CpuSurface::new(stamp(1)?.surface, extent, context.memory())?;
        full_surface.apply(&full_update)?;
        if surface.pixels() != full_surface.pixels() {
            return Err("GPU repair readback differs from complete GPU redraw".into());
        }
        println!(
            "CANVAS: phase={phase} prepare_us={prepare_us} prepared_bytes={prepared_bytes} maximum_display_channel_error={worst} repaired_pixels={} reused_pixels={} batches={} dispatches={} readback_pan_matches_full=true",
            repair.repaired_pixels,
            repair.reused_pixels,
            prepared_work.batches - before.batches,
            prepared_work.dispatches - before.dispatches
        );
    }
    Ok(7)
}
