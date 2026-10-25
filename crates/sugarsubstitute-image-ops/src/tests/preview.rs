// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preview reductions preserve explicit sampling and cannot contaminate finished pyramid reuse.
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
fn nearest_pyramids_sample_original_pixels_and_do_not_alias_finished_levels() -> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(32, 32)?;
        let (mut graph, source) = graph(context, extent)?;
        let preview = graph.pyramid_with_filter(source, Filter::Nearest)?;
        let finished = graph.pyramid(source)?;
        let output = preview.levels()[2];
        assert_ne!(graph.fingerprint(output)?, graph.fingerprint(finished.levels()[2])?);
        let mut evaluator = Evaluator::new(graph, Grid { extent }, 64, context.memory())?;
        let tile = evaluator.tile(output, 0, 0, context)?;
        for y in 0..8 {
            for x in 0..8 {
                assert_eq!(tile.pixel(x, y), Some(grid_pixel(4 * x + 3, 4 * y + 3)));
            }
        }
        let final_tile = evaluator.tile(finished.levels()[2], 0, 0, context)?;
        assert_ne!(tile.pixel(0, 0), final_tile.pixel(0, 0));
        Ok(())
    })
}
