// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Capture every clean projected source before reconstructing rounded display-only patches.
use super::{BackdropCrop, BackdropPlan};
use crate::{CpuProjection, RenderError, cpu::check};
use sugarsubstitute_contracts::execution::{JobContext, MemoryClaim};
use sugarsubstitute_image_ops::{
    AssetId, Evaluator, Extent, ImageError, Pixel, RasterSource, Region, SrgbDecoder, SrgbEncoder,
};

/// Allocation precedes its claim so actual crop storage retires before capacity becomes reusable.
struct Captured {
    plan: BackdropCrop,
    pixels: Vec<u32>,
    _memory: MemoryClaim,
}
impl Captured {
    fn new(
        frame: &CpuProjection,
        plan: BackdropCrop,
        context: &JobContext,
    ) -> Result<Self, RenderError> {
        check(context)?;
        let crop = plan.crop();
        let memory = context.memory().claim(crop.pixels() * 4 + 128).map_err(ImageError::from)?;
        let mut pixels = Vec::new();
        pixels.try_reserve_exact(crop.pixels() as usize).map_err(ImageError::from)?;
        let stride = frame.view().target().surface().width() as usize;
        for y in crop.y()..crop.bottom() {
            check(context)?;
            let start = y as usize * stride + crop.x() as usize;
            pixels.extend_from_slice(&frame.pixels()[start..start + crop.width() as usize]);
        }
        Ok(Self { plan, pixels, _memory: memory })
    }
}

/// Private immutable source: no read can observe patches already written into the projection.
struct Source<'a> {
    asset: AssetId,
    extent: Extent,
    pixels: &'a [u32],
}
impl RasterSource for Source<'_> {
    fn read(
        &mut self,
        asset: AssetId,
        region: Region,
        output: &mut [Pixel],
        context: &JobContext,
    ) -> Result<(), ImageError> {
        if asset != self.asset
            || region.intersection(self.extent.region()) != Some(region)
            || output.len() as u64 != region.pixels()
            || self.pixels.len() as u64 != self.extent.pixels()
        {
            return Err(ImageError::Invalid("Foreign presentation backdrop raster"));
        }
        for y in region.y()..region.bottom() {
            if context.cancellation_requested() {
                return Err(ImageError::Cancelled);
            }
            if context.deadline().is_some_and(|deadline| context.now() >= deadline) {
                return Err(ImageError::Deadline);
            }
            for x in region.x()..region.right() {
                output[(y - region.y()) as usize * region.width() as usize
                    + (x - region.x()) as usize] = SrgbDecoder::decode(
                    self.pixels[y as usize * self.extent.width() as usize + x as usize],
                )?;
            }
        }
        Ok(())
    }
}

impl CpuProjection {
    /// Paint at most two acrylic regions over this completed, unpublished display projection.
    /// Call once after presentation overlays and before native upload. All source crops are
    /// captured before any patch is written, so overlapping bars cannot feed blur back into blur.
    /// Canonical backings, document graphs, frame identities and camera evidence remain unchanged.
    /// Work runs only on the caller's admitted worker; reserve [`BackdropPlan::cpu_scratch_bytes`].
    /// # Errors
    /// Rejects a foreign extent and preserves memory, allocation, graph and execution-control
    /// causes. A late failure can leave unpublished display pixels partially prepared; the caller
    /// must retire that result rather than publish it. No previously shown pixels are borrowed.
    pub fn paint_backdrops(
        &mut self,
        plan: &BackdropPlan,
        context: &JobContext,
    ) -> Result<(), RenderError> {
        check(context)?;
        if plan.viewport() != self.view().target().surface() {
            return Err(RenderError::Invalid("Backdrop plan differs from projection extent"));
        }
        if plan.regions().is_empty() {
            return Ok(());
        }
        let _metadata = context
            .memory()
            .claim((2 * size_of::<Captured>() + 128) as u64)
            .map_err(ImageError::from)?;
        let mut captures = Vec::new();
        captures.try_reserve_exact(plan.regions().len()).map_err(ImageError::from)?;
        for region in plan.regions() {
            captures.push(Captured::new(self, *region, context)?);
        }
        let encoder = SrgbEncoder::new(context.memory())?;
        for (index, captured) in captures.iter().enumerate() {
            // Each display target is unique within its surface. Evaluators are private to this
            // immutable crop and never retain these ephemeral assets between frames or regions.
            let mut identity = [0_u8; 32];
            for (slot, value) in identity.chunks_exact_mut(8).zip([
                self.target().surface.get(),
                self.target().generation.get(),
                self.source().generation.get(),
                index as u64,
            ]) {
                slot.copy_from_slice(&value.to_le_bytes());
            }
            self.paint_crop(captured, AssetId(identity), &encoder, context)?;
        }
        check(context)
    }

    fn paint_crop(
        &mut self,
        captured: &Captured,
        asset: AssetId,
        encoder: &SrgbEncoder,
        context: &JobContext,
    ) -> Result<(), RenderError> {
        let crop = captured.plan.crop();
        let extent = Extent::new(crop.width(), crop.height())?;
        let source = Source { asset, extent, pixels: &captured.pixels };
        let prepared = captured.plan.graph(asset, context.memory())?;
        let mut evaluator = Evaluator::new(
            prepared.graph,
            source,
            captured.plan.retained_tiles(),
            context.memory(),
        )?;
        let stride = self.view().target().surface().width() as usize;
        for region in captured.plan.inner_region().tiles() {
            check(context)?;
            let tile = evaluator.region(prepared.output, region, context)?;
            for y in region.y()..region.bottom() {
                check(context)?;
                for x in region.x()..region.right() {
                    let coverage = captured.plan.coverage(crop.x() + x, crop.y() + y);
                    if coverage == 0.0 {
                        continue;
                    }
                    let pixel = tile.pixels()[(y - region.y()) as usize * region.width() as usize
                        + (x - region.x()) as usize];
                    let [red, green, blue, alpha] = encoder.encode(pixel)?;
                    let blurred = u32::from_be_bytes([alpha, red, green, blue]);
                    let original = captured.pixels[y as usize * crop.width() as usize + x as usize];
                    self.pixels_mut()[(crop.y() + y) as usize * stride + (crop.x() + x) as usize] =
                        masked(original, blurred, coverage);
                }
            }
        }
        Ok(())
    }
}

/// Clip the reconstructed encoded backdrop with premultiplied geometric coverage.
pub(super) fn masked(original: u32, blurred: u32, coverage: f32) -> u32 {
    let source = original.to_be_bytes();
    let adjusted = blurred.to_be_bytes();
    u32::from_be_bytes(std::array::from_fn(|index| {
        (f32::from(source[index]) * (1.0 - coverage) + f32::from(adjusted[index]) * coverage)
            .round() as u8
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BackdropRegion, tests::harness};

    /// Actual encoded crop reads and Gaussian outputs stay identical while intermediate tiles
    /// survive across streamed output requests instead of repeatedly decoding the same source.
    #[test]
    fn retained_crop_inventory_reuses_source_without_changing_streamed_pixels()
    -> harness::TestResult {
        harness::run(32 * 1024 * 1024, |context| {
            let result = (|| -> Result<(), RenderError> {
                let plan = BackdropPlan::new(
                    Extent::new(1920, 1080)?,
                    &[BackdropRegion::new([660.0, 18.0, 600.0, 40.0], 12.0, 10.0)?],
                )?;
                let crop = plan.regions()[0];
                let extent = Extent::new(crop.crop().width(), crop.crop().height())?;
                let _pixels =
                    context.memory().claim(extent.pixels() * 4 + 128).map_err(ImageError::from)?;
                let mut pixels = Vec::new();
                pixels.try_reserve_exact(extent.pixels() as usize).map_err(ImageError::from)?;
                for y in 0..extent.height() {
                    for x in 0..extent.width() {
                        pixels.push(u32::from_be_bytes([
                            255,
                            ((x * 73 + y * 29) & 255) as u8,
                            ((x * 17) ^ (y * 31)) as u8,
                            ((x / 7 + y / 13) & 255) as u8,
                        ]));
                    }
                }
                let _outputs = context
                    .memory()
                    .claim(2 * crop.inner_region().pixels() * size_of::<Pixel>() as u64 + 256)
                    .map_err(ImageError::from)?;
                let evaluate = |capacity| -> Result<_, RenderError> {
                    let asset = AssetId([9; 32]);
                    let source = Source { asset, extent, pixels: &pixels };
                    let prepared = crop.graph(asset, context.memory())?;
                    let mut evaluator =
                        Evaluator::new(prepared.graph, source, capacity, context.memory())?;
                    let mut output = Vec::new();
                    output
                        .try_reserve_exact(crop.inner_region().pixels() as usize)
                        .map_err(ImageError::from)?;
                    for region in crop.inner_region().tiles() {
                        let tile = evaluator.region(prepared.output, region, context)?;
                        output.extend_from_slice(tile.pixels());
                    }
                    Ok((output, evaluator.stats()))
                };
                let (small, repeated) = evaluate(2)?;
                let (retained, reused) = evaluate(crop.retained_tiles())?;
                assert_eq!(small, retained);
                assert!(
                    reused.source_pixels > 0 && reused.source_pixels <= extent.pixels(),
                    "Retained crop decoded source pixels repeatedly: {reused:?}"
                );
                assert!(
                    repeated.source_pixels > reused.source_pixels
                        && repeated.computed_tiles > reused.computed_tiles,
                    "Local DAG retention did not remove source/reconstruction recomputation: {repeated:?}, {reused:?}"
                );
                Ok(())
            })();
            result.map_err(|cause| ImageError::Source(Box::new(cause)))
        })
    }
}
