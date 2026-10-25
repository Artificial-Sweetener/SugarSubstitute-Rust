// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exhaustively compare GPU display lookup and half-step rounding with the canonical CPU encoder.
use std::{error::Error, sync::Arc};
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_image_gpu::{GpuDevice, GpuEvaluator};
use sugarsubstitute_image_ops::*;

const ASSET: AssetId = AssetId([29; 32]);
struct Samples;
impl RasterSource for Samples {
    fn read(
        &mut self,
        asset: AssetId,
        region: Region,
        output: &mut [Pixel],
        _: &JobContext,
    ) -> Result<(), ImageError> {
        if asset != ASSET {
            return Err(ImageError::Invalid("Unknown display qualification source"));
        }
        for y in region.y()..region.bottom() {
            for x in region.x()..region.right() {
                let index = (y * 256 + x) as f32;
                let blue = match (y * 256 + x) % 4 {
                    0 => -2.0,
                    1 => 1.4,
                    2 => 0.49999997 / 65535.0,
                    _ => 0.5 / 65535.0,
                };
                output[((y - region.y()) * region.width() + x - region.x()) as usize] =
                    Pixel::linear(
                        index / 65535.0,
                        (index + 0.5).min(65535.0) / 65535.0,
                        blue,
                        1.0,
                    )?;
            }
        }
        Ok(())
    }
}
pub(super) fn qualify(
    device: Arc<GpuDevice>,
    context: &JobContext,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let extent = Extent::new(256, 256)?;
    let mut graph = Graph::new(2, context.memory())?;
    let output = graph.add(Operation::Raster { asset: ASSET, extent })?;
    let translucent =
        graph.add(Operation::Solid { extent, color: Pixel::linear(0.3, 0.2, 0.1, 0.5)? })?;
    let mut gpu = GpuEvaluator::new(graph, Samples, 8, device, context.memory(), context)?;
    let inventory: Vec<_> = extent.region().tiles().collect();
    let prepared = gpu.display_regions(output, &inventory, context)?;
    let encoder = SrgbEncoder::new(context.memory())?;
    let _memory = context.memory().claim(128 * 128 * 16)?;
    let mut expected = vec![Pixel::TRANSPARENT; 128 * 128];
    let mut source = Samples;
    for tile in prepared.tiles() {
        source.read(ASSET, tile.region(), &mut expected, context)?;
        let actual = tile.read(context)?;
        for (actual, expected) in actual.pixels().iter().zip(&expected) {
            let [r, g, b, _] = encoder.encode(*expected)?;
            if *actual != u32::from_be_bytes([255, r, g, b]) {
                return Err("GPU sRGB transfer or half-step rounding differs from CPU".into());
            }
        }
    }
    if gpu.display_regions(translucent, &inventory, context).is_ok() {
        return Err("GPU display accepted a nonopaque composite".into());
    }
    println!(
        "DISPLAY: samples=65536 transfer_bytes_exact=true half_step_rounding_exact=true hdr_clipped=true nonopaque_rejected=true"
    );
    Ok(())
}
