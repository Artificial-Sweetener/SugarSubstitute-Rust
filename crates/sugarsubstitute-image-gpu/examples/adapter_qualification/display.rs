// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compare GPU display conversion and sparse source batching with canonical image operations.
use std::{
    error::Error,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use sugarsubstitute_contracts::execution::JobContext;
use sugarsubstitute_image_gpu::{GpuDevice, GpuEvaluator};
use sugarsubstitute_image_ops::*;

const ASSET: AssetId = AssetId([29; 32]);
#[derive(Default)]
struct BatchObservation {
    calls: AtomicUsize,
    requests: AtomicUsize,
    scalar_calls: AtomicUsize,
}
struct Samples {
    batches: Option<Arc<BatchObservation>>,
}
impl Samples {
    fn pixel(x: u32, y: u32) -> Result<Pixel, ImageError> {
        let index = (y * 256 + x) as f32;
        let blue = match (y * 256 + x) % 4 {
            0 => -2.0,
            1 => 1.4,
            2 => 0.49999997 / 65535.0,
            _ => 0.5 / 65535.0,
        };
        Pixel::linear(index / 65535.0, (index + 0.5).min(65535.0) / 65535.0, blue, 1.0)
    }
    fn fill_samples(samples: &RasterSamples, output: &mut [Pixel]) -> Result<(), ImageError> {
        if output.len() as u64 != samples.region().pixels() {
            return Err(ImageError::Invalid("Invalid display qualification sample destination"));
        }
        for (y, &row) in samples.rows().iter().enumerate() {
            for (x, &column) in samples.columns().iter().enumerate() {
                output[y * samples.columns().len() + x] = Self::pixel(column, row)?;
            }
        }
        Ok(())
    }
}
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
                output[((y - region.y()) * region.width() + x - region.x()) as usize] =
                    Self::pixel(x, y)?;
            }
        }
        Ok(())
    }
    fn read_samples(
        &mut self,
        asset: AssetId,
        samples: &RasterSamples,
        output: &mut [Pixel],
        _: &JobContext,
    ) -> Result<(), ImageError> {
        if asset != ASSET {
            return Err(ImageError::Invalid("Unknown display qualification source"));
        }
        if let Some(batches) = &self.batches {
            batches.scalar_calls.fetch_add(1, Ordering::Relaxed);
        }
        Self::fill_samples(samples, output)
    }
    fn read_sample_batch(
        &mut self,
        asset: AssetId,
        requests: &mut [RasterSampleRequest<'_>],
        _: &JobContext,
    ) -> Result<(), ImageError> {
        if asset != ASSET {
            return Err(ImageError::Invalid("Unknown display qualification source"));
        }
        validate_raster_sample_batch(requests)?;
        if let Some(batches) = &self.batches {
            batches.calls.fetch_add(1, Ordering::Relaxed);
            batches.requests.fetch_add(requests.len(), Ordering::Relaxed);
        }
        for request in requests {
            Self::fill_samples(request.samples, request.output)?;
        }
        Ok(())
    }
}
pub(super) fn qualify(
    device: Arc<GpuDevice>,
    context: &JobContext,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let extent = Extent::new(256, 256)?;
    let mut graph = Graph::new(3, context.memory())?;
    let output = graph.add(Operation::Raster { asset: ASSET, extent })?;
    let sparse_sampling = Sampling::new([4.0, 4.0], [0.0; 2])?.with_filter(Filter::Nearest);
    let sparse =
        graph.add(Operation::Resample { input: output, extent, sampling: sparse_sampling })?;
    let translucent =
        graph.add(Operation::Solid { extent, color: Pixel::linear(0.3, 0.2, 0.1, 0.5)? })?;
    let batches = Arc::new(BatchObservation::default());
    let mut gpu = GpuEvaluator::new(
        graph,
        Samples { batches: Some(batches.clone()) },
        8,
        device,
        context.memory(),
        context,
    )?;
    let inventory: Vec<_> = extent.region().tiles().collect();
    let prepared = gpu.display_regions(output, &inventory, context)?;
    let encoder = SrgbEncoder::new(context.memory())?;
    let _memory = context.memory().claim(128 * 128 * 16)?;
    let mut expected = vec![Pixel::TRANSPARENT; 128 * 128];
    let mut source = Samples { batches: None };
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
    drop(prepared);
    let scalar = gpu.display_regions(sparse, &inventory[..1], context)?;
    if batches.calls.load(Ordering::Relaxed) != 0
        || batches.scalar_calls.load(Ordering::Relaxed) != 1
    {
        return Err("One GPU sparse region did not stay on its scalar source path".into());
    }
    drop(scalar);
    gpu.clear_tiles();
    let prepared = gpu.display_regions(sparse, &inventory, context)?;
    if batches.calls.load(Ordering::Relaxed) != 2
        || batches.requests.load(Ordering::Relaxed) != inventory.len()
        || batches.scalar_calls.load(Ordering::Relaxed) != 1
    {
        return Err("GPU sparse display did not batch each complete horizontal source row".into());
    }
    for tile in prepared.tiles() {
        let samples = RasterSamples::new(extent, tile.region(), sparse_sampling)?;
        Samples::fill_samples(&samples, &mut expected)?;
        let actual = tile.read(context)?;
        for (actual, expected) in actual.pixels().iter().zip(&expected) {
            let [r, g, b, _] = encoder.encode(*expected)?;
            if *actual != u32::from_be_bytes([255, r, g, b]) {
                return Err("GPU sparse row batch changed canonical display pixels".into());
            }
        }
    }
    drop(prepared);
    if gpu.display_regions(translucent, &inventory, context).is_ok() {
        return Err("GPU display accepted a nonopaque composite".into());
    }
    println!(
        "DISPLAY: samples=65536 transfer_bytes_exact=true half_step_rounding_exact=true hdr_clipped=true sparse_scalar_reads=1 sparse_row_batches=2 sparse_requests=4 nonopaque_rejected=true"
    );
    Ok(())
}
