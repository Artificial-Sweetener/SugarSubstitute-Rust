// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compare navigation reconstruction with independent linear-light scalar interpolation.
use super::*;
use crate::SamplingAxis;

#[test]
fn bilinear_tiles_match_scalar_pixels_across_boundaries_and_clamped_edges() -> TestResult {
    run(MEMORY, |context| {
        let source = Extent::new(257, 131)?;
        let target = Extent::new(260, 129)?;
        let (mut graph, input) = graph(context, source)?;
        let sampling = Sampling::new([0.9, 1.1], [-0.3, 2.2])?.with_filter(Filter::Bilinear);
        let output = graph.add(Operation::Resample { input, extent: target, sampling })?;
        let mut evaluator = Evaluator::new(graph, Grid { extent: source }, 32, context.memory())?;
        for region in target.region().tiles() {
            let tile = evaluator.region(output, region, context)?;
            for y in region.y()..region.bottom() {
                for x in region.x()..region.right() {
                    let sx = ((f64::from(x) + 0.5) * 0.9 - 0.8).clamp(0.0, 256.0);
                    let sy = ((f64::from(y) + 0.5) * 1.1 + 1.7).clamp(0.0, 130.0);
                    let left = sx.floor() as u32;
                    let top = sy.floor() as u32;
                    let fx = (sx - f64::from(left)) as f32;
                    let fy = (sy - f64::from(top)) as f32;
                    let corners = [
                        grid_pixel(left, top),
                        grid_pixel((left + 1).min(256), top),
                        grid_pixel(left, (top + 1).min(130)),
                        grid_pixel((left + 1).min(256), (top + 1).min(130)),
                    ];
                    let actual =
                        tile.pixel(x, y).ok_or(ImageError::Invalid("Missing bilinear pixel"))?;
                    for (channel, actual) in actual.channels().into_iter().enumerate() {
                        let upper = corners[0].channels()[channel] * (1.0 - fx)
                            + corners[1].channels()[channel] * fx;
                        let lower = corners[2].channels()[channel] * (1.0 - fx)
                            + corners[3].channels()[channel] * fx;
                        let expected = upper * (1.0 - fy) + lower * fy;
                        assert!((actual - expected).abs() < 2e-6, "({x}, {y}), channel {channel}");
                    }
                }
            }
        }
        Ok(())
    })
}

#[test]
fn bilinear_support_is_normalized_and_has_at_most_two_taps_per_axis() -> TestResult {
    for source in [1, 2, 257] {
        for offset in [-1000.0, -0.25, 0.0, 0.375, 1000.0] {
            let sampling = Sampling::new([1.25; 2], [offset; 2])?.with_filter(Filter::Bilinear);
            for x in [0, 1, 127, 128, 256] {
                let kernel = sampling.axis_kernel(SamplingAxis::Horizontal, x, source)?;
                assert!(kernel.taps().len() <= 2);
                assert!(
                    kernel.taps().iter().all(|tap| tap.coordinate() < source && tap.weight() > 0.0)
                );
                assert!(
                    (kernel.taps().iter().map(|tap| tap.weight()).sum::<f32>() - 1.0).abs() < 1e-6
                );
            }
        }
    }
    Ok(())
}

#[test]
fn navigation_keeps_canonical_pyramids_and_source_pixel_inspection() -> TestResult {
    for zoom in [0.001, 0.5, 1.0, 2.0] {
        let intent = Reconstruction::navigation_for_display_zoom(zoom)?;
        assert_eq!(intent.filter(), Filter::Bilinear);
        assert!(intent.uses_canonical_pyramid());
        assert!(!intent.is_preview());
    }
    assert_eq!(Reconstruction::navigation_for_display_zoom(2.01)?.filter(), Filter::Nearest);
    for invalid in [0.0, -1.0, f64::INFINITY, f64::NAN] {
        assert!(Reconstruction::navigation_for_display_zoom(invalid).is_err());
    }
    Ok(())
}

#[test]
fn bilinear_and_finished_sampling_have_distinct_reusable_graph_identities() -> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(257, 131)?;
        let (mut graph, input) = graph(context, extent)?;
        let sampling = Sampling::new([1.0; 2], [0.25; 2])?;
        let mut keys = Vec::new();
        for filter in [Filter::Lanczos3, Filter::Nearest, Filter::Bilinear] {
            let node = graph.add(Operation::Resample {
                input,
                extent,
                sampling: sampling.with_filter(filter),
            })?;
            keys.push(graph.fingerprint(node)?);
        }
        assert_ne!(keys[0], keys[2]);
        assert_ne!(keys[1], keys[2]);
        Ok(())
    })
}
