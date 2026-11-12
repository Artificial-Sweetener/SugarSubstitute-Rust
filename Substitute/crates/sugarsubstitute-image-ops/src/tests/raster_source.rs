// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Sparse source batches validate every destination before invoking default scalar reads.

use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

struct CountingSource {
    reads: usize,
}

impl RasterSource for CountingSource {
    fn read(
        &mut self,
        _: AssetId,
        region: Region,
        output: &mut [Pixel],
        _: &JobContext,
    ) -> Result<(), ImageError> {
        self.reads += 1;
        output.fill(grid_pixel(region.x(), region.y()));
        Ok(())
    }
}

struct PartialWarmSource {
    sparse_reads: Arc<AtomicU64>,
}
impl RasterSource for PartialWarmSource {
    fn read(
        &mut self,
        _: AssetId,
        _: Region,
        output: &mut [Pixel],
        _: &JobContext,
    ) -> Result<(), ImageError> {
        output.fill(Pixel::TRANSPARENT);
        Ok(())
    }
    fn read_samples(
        &mut self,
        _: AssetId,
        samples: &RasterSamples,
        output: &mut [Pixel],
        _: &JobContext,
    ) -> Result<(), ImageError> {
        if output.len() as u64 != samples.region().pixels() {
            return Err(ImageError::Invalid("Invalid partial-warm sparse destination"));
        }
        self.sparse_reads.fetch_add(1, Ordering::Relaxed);
        output.fill(Pixel::TRANSPARENT);
        Ok(())
    }
}

#[test]
fn partial_warm_batch_pins_its_cached_leaf_while_the_missing_leaf_stays_scalar() -> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(1024, 128)?;
        let (mut warmed, source) = graph(context, extent)?;
        warmed.pyramid(source)?;
        let sparse_reads = Arc::new(AtomicU64::new(0));
        let mut evaluator = Evaluator::new(
            warmed,
            PartialWarmSource { sparse_reads: Arc::clone(&sparse_reads) },
            4,
            context.memory(),
        )?;
        for x in 0..4 {
            drop(evaluator.tile(source, x, 0, context)?);
        }
        assert_eq!(evaluator.stats().resident_tiles, 4);

        let target = Extent::new(256, 128)?;
        let (mut preview, source) = graph(context, extent)?;
        let output = preview.add(Operation::Resample {
            input: source,
            extent: target,
            sampling: Sampling::resize(extent, target).with_filter(Filter::Nearest),
        })?;
        evaluator.replace_graph(preview);
        let first = Region::new(0, 0, 128, 128)?;
        let second = Region::new(128, 0, 128, 128)?;
        drop(evaluator.region(output, first, context)?);
        assert_eq!(sparse_reads.load(Ordering::Relaxed), 1);

        evaluator.prepare_sparse_regions(output, &[first, second], context)?;
        drop(evaluator.region(output, second, context)?);
        drop(evaluator.region(output, first, context)?);
        assert_eq!(sparse_reads.load(Ordering::Relaxed), 2);
        assert!(evaluator.stats().resident_tiles <= 4);
        evaluator.release_sparse_batch();
        Ok(())
    })
}

#[test]
fn default_batch_preserves_request_order_and_rejects_all_invalid_destinations_before_writes()
-> TestResult {
    run(MEMORY, |context| {
        let source = Extent::new(32, 16)?;
        let sampling = Sampling::new([2.0, 1.0], [0.0; 2])?.with_filter(Filter::Nearest);
        let first = RasterSamples::new(source, Region::new(0, 0, 2, 1)?, sampling)?;
        let second = RasterSamples::new(source, Region::new(2, 0, 2, 1)?, sampling)?;
        let sentinel = Pixel::linear(0.75, 0.5, 0.25, 1.0)?;
        let mut first_output = [sentinel; 2];
        let mut invalid_output = [sentinel; 1];
        let mut requests = [
            RasterSampleRequest { samples: &first, output: &mut first_output },
            RasterSampleRequest { samples: &second, output: &mut invalid_output },
        ];
        let mut raster = CountingSource { reads: 0 };
        assert!(raster.read_sample_batch(ASSET, &mut requests, context).is_err());
        assert_eq!(raster.reads, 0);
        assert_eq!(first_output, [sentinel; 2]);
        assert_eq!(invalid_output, [sentinel; 1]);

        let empty: [RasterSampleRequest<'_>; 0] = [];
        assert!(validate_raster_sample_batch(&empty).is_err());
        let other = RasterSamples::new(Extent::new(64, 16)?, Region::new(0, 0, 2, 1)?, sampling)?;
        let mut other_output = [sentinel; 2];
        let mixed = [
            RasterSampleRequest { samples: &first, output: &mut first_output },
            RasterSampleRequest { samples: &other, output: &mut other_output },
        ];
        assert!(validate_raster_sample_batch(&mixed).is_err());
        let mut excessive_outputs = vec![[sentinel; 2]; MAX_RASTER_SAMPLE_BATCH + 1];
        let excessive: Vec<_> = excessive_outputs
            .iter_mut()
            .map(|output| RasterSampleRequest { samples: &first, output })
            .collect();
        assert!(validate_raster_sample_batch(&excessive).is_err());

        let mut first_output = [Pixel::TRANSPARENT; 2];
        let mut second_output = [Pixel::TRANSPARENT; 2];
        let mut requests = [
            RasterSampleRequest { samples: &first, output: &mut first_output },
            RasterSampleRequest { samples: &second, output: &mut second_output },
        ];
        raster.read_sample_batch(ASSET, &mut requests, context)?;
        assert_eq!(raster.reads, 4);
        assert_eq!(first_output, [grid_pixel(1, 0), grid_pixel(3, 0)]);
        assert_eq!(second_output, [grid_pixel(5, 0), grid_pixel(7, 0)]);
        Ok(())
    })
}
