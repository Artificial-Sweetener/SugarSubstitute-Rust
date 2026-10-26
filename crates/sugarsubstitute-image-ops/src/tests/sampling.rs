// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Global sampling phase, clamped edge dependencies and non-destructive adjustment coverage.
use super::*;

#[test]
fn finished_display_reconstruction_changes_only_strictly_above_two_x() -> TestResult {
    assert_eq!(Reconstruction::Preview.filter(), Filter::Nearest);
    assert!(Reconstruction::Preview.is_preview());
    assert!(!Reconstruction::Preview.uses_canonical_pyramid());
    assert!(Reconstruction::Finished(Filter::Nearest).uses_canonical_pyramid());
    assert!(!Reconstruction::Finished(Filter::Nearest).is_preview());
    assert_eq!(
        Reconstruction::finished_for_display_zoom(0.125)?,
        Reconstruction::Finished(Filter::Lanczos3)
    );
    assert_eq!(
        Reconstruction::finished_for_display_zoom(2.0)?,
        Reconstruction::Finished(Filter::Lanczos3)
    );
    assert_eq!(
        Reconstruction::finished_for_display_zoom(f64::from_bits(2.0_f64.to_bits() + 1))?,
        Reconstruction::Finished(Filter::Nearest)
    );
    assert_eq!(
        Reconstruction::finished_for_display_zoom(400.0)?,
        Reconstruction::Finished(Filter::Nearest)
    );
    for invalid in [0.0, -1.0, f64::INFINITY, f64::NAN] {
        assert!(Reconstruction::finished_for_display_zoom(invalid).is_err());
    }
    Ok(())
}

#[test]
fn shared_axis_kernels_preserve_phase_clamping_and_normalized_bounded_support() -> TestResult {
    let sampling = Sampling::new([0.5, 2.0], [-0.25, 3.5])?.with_filter(Filter::Nearest);
    let horizontal = sampling.axis_kernel(SamplingAxis::Horizontal, 128, 4097)?;
    let vertical = sampling.axis_kernel(SamplingAxis::Vertical, 0, 4097)?;
    assert_eq!(horizontal.taps().len(), 1);
    assert_eq!(horizontal.taps()[0].coordinate(), 64);
    assert_eq!(vertical.taps()[0].coordinate(), 4);
    assert_eq!(horizontal.taps()[0].weight(), 1.0);
    let sampling = Sampling::resize(Extent::new(4097, 4097)?, Extent::new(2049, 2049)?);
    for output in [0, 1, 127, 128, 2048] {
        let kernel = sampling.axis_kernel(SamplingAxis::Horizontal, output, 4097)?;
        assert!(kernel.taps().len() <= MAX_SAMPLING_TAPS);
        assert!(kernel.taps().iter().all(|tap| tap.coordinate() < 4097));
        let sum: f32 = kernel.taps().iter().map(|tap| tap.weight()).sum();
        assert!((sum - 1.0).abs() < 1e-6);
    }
    Ok(())
}

#[test]
fn shared_axis_kernels_reject_unbounded_steps_and_preserve_a_single_pixel_axis() -> TestResult {
    let sampling = Sampling::new([128.0; 2], [0.0; 2])?;
    assert!(sampling.axis_kernel(SamplingAxis::Horizontal, 0, 4097).is_err());
    assert!(sampling.axis_kernel(SamplingAxis::Horizontal, 0, 0).is_err());
    let kernel = sampling.axis_kernel(SamplingAxis::Horizontal, 17, 1)?;
    assert_eq!(kernel.taps().len(), 1);
    assert_eq!(kernel.taps()[0].coordinate(), 0);
    assert_eq!(kernel.taps()[0].weight(), 1.0);
    let extreme = Sampling::new([f64::MAX; 2], [f64::MAX; 2])?;
    for filter in [Filter::Nearest, Filter::Lanczos3, Filter::Bilinear] {
        assert!(
            extreme.with_filter(filter).axis_kernel(SamplingAxis::Vertical, u32::MAX, 1).is_err()
        );
    }
    Ok(())
}

#[test]
fn nearest_preview_has_exact_global_phase_bounded_dependencies_and_distinct_content_keys()
-> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(257, 131)?;
        let target = Extent::new(260, 129)?;
        let (mut graph, source) = graph(context, extent)?;
        let sampling = Sampling::new([0.9, 1.1], [-0.3, 2.2])?;
        let finished =
            graph.add(Operation::Resample { input: source, extent: target, sampling })?;
        let preview = graph.add(Operation::Resample {
            input: source,
            extent: target,
            sampling: sampling.with_filter(Filter::Nearest),
        })?;
        assert_ne!(graph.fingerprint(preview)?, graph.fingerprint(finished)?);
        let mut evaluator = Evaluator::new(graph, Grid { extent }, 16, context.memory())?;
        for ty in 0..2 {
            for tx in 0..3 {
                let tile = evaluator.tile(preview, tx, ty, context)?;
                let region = tile.region();
                let dependency = evaluator.graph().required_inputs(preview, region)?[0]
                    .ok_or(ImageError::Invalid("Missing preview dependency"))?
                    .1;
                for y in region.y()..region.bottom() {
                    for x in region.x()..region.right() {
                        let sx =
                            ((f64::from(x) + 0.5) * 0.9 - 0.3).floor().clamp(0.0, 256.0) as u32;
                        let sy =
                            ((f64::from(y) + 0.5) * 1.1 + 2.2).floor().clamp(0.0, 130.0) as u32;
                        assert!(dependency.contains(sx, sy));
                        assert_eq!(tile.pixel(x, y), Some(grid_pixel(sx, sy)));
                        let dirty = evaluator
                            .graph()
                            .invalidated_by(preview, source, Region::new(sx, sy, 1, 1)?)?
                            .ok_or(ImageError::Invalid("Preview invalidation missed its sample"))?;
                        assert!(dirty.contains(x, y));
                    }
                }
            }
        }
        let before = evaluator.stats().computed_tiles;
        drop(evaluator.tile(finished, 0, 0, context)?);
        assert!(evaluator.stats().computed_tiles > before);
        Ok(())
    })
}

#[test]
fn integer_sampling_preserves_exact_pixels_and_clamped_edges() -> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(31, 23)?;
        let target = Extent::new(128, 64)?;
        let (mut graph, source) = graph(context, extent)?;
        let sampling = Sampling::new([1.0; 2], [-10.0, -7.0])?;
        let output = graph.add(Operation::Resample { input: source, extent: target, sampling })?;
        let mut evaluator = Evaluator::new(graph, Grid { extent }, 8, context.memory())?;
        let tile = evaluator.tile(output, 0, 0, context)?;
        for y in 0..64 {
            for x in 0..128 {
                let sx = (i64::from(x) - 10).clamp(0, 30) as u32;
                let sy = (i64::from(y) - 7).clamp(0, 22) as u32;
                assert_eq!(tile.pixel(x, y), Some(grid_pixel(sx, sy)));
            }
        }
        assert!(Sampling::new([0.0, 1.0], [0.0; 2]).is_err());
        assert!(Sampling::new([1.0; 2], [f64::NAN, 0.0]).is_err());
        Ok(())
    })
}

#[test]
fn one_pixel_pyramid_axes_keep_bounded_work_under_extreme_minification() -> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(1, 128)?;
        let (mut graph, source) = graph(context, extent)?;
        let sampling = Sampling::new([1e9, 1.0], [-1e200, 0.0])?;
        let output = graph.add(Operation::Resample {
            input: source,
            extent: Extent::new(128, 128)?,
            sampling,
        })?;
        let mut evaluator = Evaluator::new(graph, Grid { extent }, 8, context.memory())?;
        let tile = evaluator.tile(output, 0, 0, context)?;
        for y in 0..128 {
            for x in 0..128 {
                assert_eq!(tile.pixel(x, y), Some(grid_pixel(0, y)));
            }
        }
        assert_eq!(evaluator.stats().source_pixels, 128);
        Ok(())
    })
}

#[test]
fn clamped_edge_changes_invalidate_samples_outside_the_original_extent() -> TestResult {
    run(MEMORY, |context| {
        let (mut graph, source) = graph(context, Extent::new(16, 16)?)?;
        let target = Extent::new(128, 16)?;
        let sampling = Sampling::new([1.0; 2], [-500.0, 0.0])?;
        let output = graph.add(Operation::Resample { input: source, extent: target, sampling })?;
        let changed = Region::new(0, 1, 1, 1)?;
        let dirty = graph
            .invalidated_by(output, source, changed)?
            .ok_or(ImageError::Invalid("Missing clamped dirty region"))?;
        for x in 0..128 {
            assert!(dirty.contains(x, 1));
        }
        Ok(())
    })
}

#[test]
fn graph_replacement_reuses_unchanged_source_tiles_without_old_node_aliases() -> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(128, 128)?;
        let (first, source) = graph(context, extent)?;
        let mut evaluator = Evaluator::new(first, Grid { extent }, 8, context.memory())?;
        let original = evaluator.tile(source, 0, 0, context)?;
        let (mut next, next_source) = graph(context, extent)?;
        let adjusted = next.add(Operation::Exposure { input: next_source, stops: 1.0 })?;
        evaluator.replace_graph(next);
        assert!(evaluator.tile(source, 0, 0, context).is_err());
        let reused = evaluator.tile(next_source, 0, 0, context)?;
        assert!(Arc::ptr_eq(&original, &reused));
        evaluator.tile(adjusted, 0, 0, context)?;
        assert_eq!(evaluator.stats().source_pixels, 128 * 128);
        Ok(())
    })
}

#[test]
fn masked_adjustment_interpolation_preserves_translucent_source_alpha() -> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(1, 1)?;
        let mut graph = Graph::new(8, context.memory())?;
        let color = Pixel::linear(0.4, 0.2, 0.1, 0.25)?;
        let original = graph.add(Operation::Solid { extent, color })?;
        let adjusted = graph.add(Operation::Exposure { input: original, stops: 1.0 })?;
        let mask =
            graph.add(Operation::Solid { extent, color: Pixel::linear(0.0, 0.0, 0.0, 0.5)? })?;
        let output = graph.add(Operation::Mix { original, adjusted, mask, opacity: 0.5 })?;
        let mut evaluator = Evaluator::new(graph, Grid { extent }, 8, context.memory())?;
        let result = evaluator.tile(output, 0, 0, context)?;
        let expected = [0.125, 0.0625, 0.03125, 0.25];
        for (actual, expected) in result.pixels()[0].channels().into_iter().zip(expected) {
            assert!((actual - expected).abs() < 1e-7);
        }
        Ok(())
    })
}

#[test]
fn fully_masked_tiles_do_not_load_hidden_source_pixels() -> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(1_000_000_000, 1_000_000_000)?;
        let (mut graph, source) = graph(context, extent)?;
        let mask = graph.add(Operation::Solid { extent, color: Pixel::TRANSPARENT })?;
        let output = graph.add(Operation::Mask { input: source, mask })?;
        let mut evaluator = Evaluator::new(graph, Grid { extent }, 8, context.memory())?;
        let result = evaluator.tile(output, 7_000_000, 7_000_000, context)?;
        assert!(result.pixels().iter().all(|pixel| *pixel == Pixel::TRANSPARENT));
        assert_eq!(evaluator.stats().source_pixels, 0);
        Ok(())
    })
}

#[test]
fn display_lookup_tracks_srgb_transfer_within_one_output_level() -> TestResult {
    run(MEMORY, |context| {
        let encoder = SrgbEncoder::new(context.memory())?;
        assert_eq!(encoder.table().len(), 65536);
        for index in 0..=1000 {
            let linear = index as f32 / 1000.0;
            let actual = encoder.encode(Pixel::linear(linear, linear, linear, 1.0)?)?[0];
            let reference = if linear <= 0.0031308 {
                12.92 * f64::from(linear)
            } else {
                1.055 * f64::from(linear).powf(1.0 / 2.4) - 0.055
            };
            assert!((i32::from(actual) - (reference * 255.0).round() as i32).abs() <= 1);
        }
        assert!(encoder.encode(Pixel::TRANSPARENT).is_err());
        assert_eq!(encoder.encode(Pixel::linear(-0.5, 4.0, 1.0, 1.0)?)?, [0, 255, 255, 255]);
        Ok(())
    })
}

#[test]
fn display_quantization_preserves_reference_bytes_around_every_half_step() -> TestResult {
    run(MEMORY, |context| {
        let encoder = SrgbEncoder::new(context.memory())?;
        let verify = |linear: f32| -> Result<(), ImageError> {
            let index = (linear.clamp(0.0, 1.0) * 65535.0).round() as usize;
            let expected = encoder.table()[index];
            assert_eq!(
                SrgbEncoder::encode_constant(Pixel::linear(linear, linear, linear, 1.0)?)?,
                [expected, expected, expected, 255],
            );
            assert_eq!(
                encoder.encode(Pixel::linear(linear, linear, linear, 1.0)?)?,
                [expected, expected, expected, 255],
                "Linear sample bits {:08x}",
                linear.to_bits()
            );
            Ok(())
        };
        for index in 0..65535 {
            let threshold = ((f64::from(index) + 0.5) / 65535.0) as f32;
            for bits in threshold.to_bits() - 1..=threshold.to_bits() + 1 {
                verify(f32::from_bits(bits))?;
            }
        }
        for linear in [-f32::MAX, -1.0, -0.0, 0.0, f32::from_bits(1), 1.0, f32::MAX] {
            verify(linear)?;
        }
        Ok(())
    })
}
