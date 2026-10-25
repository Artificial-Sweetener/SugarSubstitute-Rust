// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Integer output translations retain source indices, floating coefficients and conservative damage.
use super::*;
use crate::{RasterSamples, SamplingAxis};

#[test]
fn translated_reconstruction_keeps_every_sampling_tap_bit_identical() -> TestResult {
    run(MEMORY, |_context| {
        for filter in [Filter::Nearest, Filter::Lanczos3] {
            for scale in [0.37, 1.0, 1.21, 1.37, 2.0] {
                let original = Sampling::new([scale, scale], [-3.25, 19.75])?.with_filter(filter);
                for translation in [[13, -7], [-31, 17]] {
                    let moved = original.with_output_translation(translation);
                    for (index, axis) in
                        [SamplingAxis::Horizontal, SamplingAxis::Vertical].into_iter().enumerate()
                    {
                        for position in [128_u32, 159, 1_000_003, 900_000_001] {
                            let next = (i64::from(position) + i64::from(translation[index])) as u32;
                            let before = original.axis_kernel(axis, position, 1_000_000_000)?;
                            let after = moved.axis_kernel(axis, next, 1_000_000_000)?;
                            assert_eq!(before.taps().len(), after.taps().len());
                            for (before, after) in before.taps().iter().zip(after.taps()) {
                                assert_eq!(before.coordinate(), after.coordinate());
                                assert_eq!(before.weight().to_bits(), after.weight().to_bits());
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    })
}

#[test]
fn sparse_translation_cannot_cross_the_neighboring_source_pixel_at_a_rounding_boundary()
-> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(65536, 16384)?;
        let original = Sampling::new([20.48; 2], [0.0; 2])?.with_filter(Filter::Nearest);
        let moved = original.with_output_translation([13, 0]);
        let before = Region::new(1587, 1, 4, 4)?;
        let after = Region::new(1600, 1, 4, 4)?;
        let old_samples = RasterSamples::new(extent, before, original)?;
        let new_samples = RasterSamples::new(extent, after, moved)?;
        assert_eq!(old_samples.columns()[0], 32512);
        assert_eq!(old_samples.columns(), new_samples.columns());
        assert_eq!(old_samples.rows(), new_samples.rows());
        let (mut graph, source) = graph(context, extent)?;
        let output = Extent::new(3840, 800)?;
        let first =
            graph.add(Operation::Resample { input: source, extent: output, sampling: original })?;
        let second =
            graph.add(Operation::Resample { input: source, extent: output, sampling: moved })?;
        assert_ne!(graph.fingerprint(first)?, graph.fingerprint(second)?);
        let mut evaluator = Evaluator::new(graph, Grid { extent }, 16, context.memory())?;
        let old = evaluator.region(first, before, context)?;
        let new = evaluator.region(second, after, context)?;
        assert_eq!(old.pixels(), new.pixels());
        Ok(())
    })
}

#[test]
fn translated_required_regions_and_invalidations_include_every_nonzero_sampling_tap() -> TestResult
{
    run(MEMORY, |context| {
        let extent = Extent::new(257, 131)?;
        let target = Extent::new(256, 128)?;
        for filter in [Filter::Nearest, Filter::Lanczos3] {
            let sampling = Sampling::new([1.37, 0.91], [-3.25, 19.75])?
                .with_filter(filter)
                .with_output_translation([13, -7]);
            let (mut graph, source) = graph(context, extent)?;
            let output =
                graph.add(Operation::Resample { input: source, extent: target, sampling })?;
            let region = Region::new(0, 0, 128, 128)?;
            let required = graph.required_inputs(output, region)?[0]
                .ok_or(ImageError::Invalid("Missing translated source dependency"))?
                .1;
            for (axis, length, start, end) in [
                (SamplingAxis::Horizontal, extent.width(), required.x(), required.right()),
                (SamplingAxis::Vertical, extent.height(), required.y(), required.bottom()),
            ] {
                for position in 0..128 {
                    for tap in sampling.axis_kernel(axis, position, length)?.taps() {
                        assert!((start..end).contains(&tap.coordinate()));
                    }
                }
            }
            for changed in [[0, 0], [130, 65], [256, 130]] {
                let invalidated = graph.invalidated_by(
                    output,
                    source,
                    Region::new(changed[0], changed[1], 1, 1)?,
                )?;
                let mut columns = [false; 256];
                let mut rows = [false; 128];
                for (x, hit) in columns.iter_mut().enumerate() {
                    *hit = sampling
                        .axis_kernel(SamplingAxis::Horizontal, x as u32, extent.width())?
                        .taps()
                        .iter()
                        .any(|tap| tap.coordinate() == changed[0] && tap.weight() != 0.0);
                }
                for (y, hit) in rows.iter_mut().enumerate() {
                    *hit = sampling
                        .axis_kernel(SamplingAxis::Vertical, y as u32, extent.height())?
                        .taps()
                        .iter()
                        .any(|tap| tap.coordinate() == changed[1] && tap.weight() != 0.0);
                }
                let mut influenced = 0;
                for (y, hit_y) in rows.into_iter().enumerate() {
                    for (x, hit_x) in columns.into_iter().enumerate() {
                        if hit_x && hit_y {
                            influenced += 1;
                            let pixel = Region::new(x as u32, y as u32, 1, 1)?;
                            assert_eq!(
                                invalidated.and_then(|bounds| bounds.intersection(pixel)),
                                Some(pixel)
                            );
                        }
                    }
                }
                if changed == [130, 65] {
                    assert!(influenced > 0);
                }
            }
        }
        Ok(())
    })
}
