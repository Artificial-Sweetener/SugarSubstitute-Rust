// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Nearest reconstruction preserves explicit sampling without changing canonical pyramid reduction.
use super::*;

#[test]
fn sparse_nearest_reads_only_requested_original_samples_across_repairs_and_edges() -> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(16384, 8192)?;
        let (mut graph, source) = graph(context, extent)?;
        let sampling =
            Sampling::new([33.75, 39.25], [-100.5, -60.25])?.with_filter(Filter::Nearest);
        let output = graph.add(Operation::Resample {
            input: source,
            extent: Extent::new(513, 257)?,
            sampling,
        })?;
        let exposure = graph.add(Operation::Exposure { input: source, stops: 1.0 })?;
        assert!(
            graph
                .add(Operation::Resample {
                    input: exposure,
                    extent: Extent::new(513, 257)?,
                    sampling
                })
                .is_err()
        );
        let mut evaluator = Evaluator::new(graph, Grid { extent }, 16, context.memory())?;
        let regions = [
            Region::new(0, 0, 128, 128)?,
            Region::new(127, 125, 7, 9)?,
            Region::new(500, 250, 13, 7)?,
        ];
        let mut requested = 0;
        for region in regions {
            let tile = evaluator.region(output, region, context)?;
            requested += region.pixels();
            for y in region.y()..region.bottom() {
                for x in region.x()..region.right() {
                    let sx = (((f64::from(x) + 0.5) * 33.75 - 100.5).floor() as i64).clamp(0, 16383)
                        as u32;
                    let sy = (((f64::from(y) + 0.5) * 39.25 - 60.25).floor() as i64).clamp(0, 8191)
                        as u32;
                    assert_eq!(tile.pixel(x, y), Some(grid_pixel(sx, sy)));
                }
            }
        }
        assert_eq!(evaluator.stats().source_pixels, requested);
        let changed = Region::new(4303, 4904, 1, 1)?;
        let influenced = evaluator.graph().invalidated_by(output, source, changed)?;
        let pixel = Region::new(130, 126, 1, 1)?;
        assert_eq!(influenced.and_then(|region| region.intersection(pixel)), Some(pixel));
        Ok(())
    })
}

#[test]
fn canonical_pyramids_feed_distinct_nearest_final_reconstruction() -> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(32, 32)?;
        let (mut graph, source) = graph(context, extent)?;
        let pyramid = graph.pyramid(source)?;
        let reduced = pyramid.levels()[2];
        let Operation::Resample { sampling: reduction, .. } = graph.operation(reduced)? else {
            return Err(ImageError::Invalid("Canonical pyramid level is not a reduction"));
        };
        assert_eq!(reduction.filter(), Filter::Lanczos3);
        let target = Extent::new(16, 16)?;
        let sampling = Sampling::resize(graph.extent(reduced)?, target);
        let nearest = graph.add(Operation::Resample {
            input: reduced,
            extent: target,
            sampling: sampling.with_filter(Filter::Nearest),
        })?;
        let finished =
            graph.add(Operation::Resample { input: reduced, extent: target, sampling })?;
        assert_ne!(graph.fingerprint(nearest)?, graph.fingerprint(finished)?);
        let mut evaluator = Evaluator::new(graph, Grid { extent }, 64, context.memory())?;
        let reduced_tile = evaluator.tile(reduced, 0, 0, context)?;
        let nearest_tile = evaluator.tile(nearest, 0, 0, context)?;
        for y in 0..16 {
            for x in 0..16 {
                assert_eq!(nearest_tile.pixel(x, y), reduced_tile.pixel(x / 2, y / 2));
            }
        }
        let finished_tile = evaluator.tile(finished, 0, 0, context)?;
        assert!(
            nearest_tile
                .pixels()
                .iter()
                .zip(finished_tile.pixels())
                .any(|(nearest, finished)| nearest != finished),
            "Final nearest and Lanczos reconstruction unexpectedly produced identical pixels"
        );
        Ok(())
    })
}
