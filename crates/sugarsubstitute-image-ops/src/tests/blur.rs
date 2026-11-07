// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Gaussian behavior across tile edges, image edges, alpha, bounded work and structural reuse.
use super::*;

#[test]
fn gaussian_parameters_are_bounded_and_zero_is_canonical() -> TestResult {
    for sigma in [-1.0, f32::NAN, f32::INFINITY, MAX_GAUSSIAN_SIGMA + 0.1] {
        assert!(GaussianBlur::new(sigma).is_err());
    }
    assert_eq!(GaussianBlur::new(-0.0)?.sigma().to_bits(), 0);
    let blur = GaussianBlur::new(MAX_GAUSSIAN_SIGMA)?;
    assert_eq!(blur.radius(), 72);
    let weights = blur.weights();
    assert!((weights.iter().sum::<f32>() - 1.0).abs() < 1e-6);
    for index in 0..MAX_GAUSSIAN_TAPS {
        assert_eq!(weights[index], weights[MAX_GAUSSIAN_TAPS - index - 1]);
    }
    Ok(())
}

#[test]
fn gaussian_spatial_dependencies_and_damage_include_only_the_clipped_halo() -> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(1000, 1000)?;
        let (mut graph, source) = graph(context, extent)?;
        let blurred =
            graph.add(Operation::GaussianBlur { input: source, blur: GaussianBlur::new(2.0)? })?;
        let requested = Region::new(126, 3, 5, 4)?;
        let halo = Region::new(120, 0, 17, 13)?;
        assert_eq!(graph.required_inputs(blurred, requested)?[0], Some((source, halo)));
        assert_eq!(graph.invalidated_by(blurred, source, requested)?, Some(halo));
        assert_eq!(
            graph.invalidated_by(blurred, source, Region::new(999, 999, 1, 1)?)?,
            Some(Region::new(993, 993, 7, 7)?)
        );
        Ok(())
    })
}

/// Direct two-dimensional f64 reference, independent of the separable implementation and tables.
fn reference(x: u32, y: u32, extent: Extent, sigma: f64) -> [f32; 4] {
    let radius = (sigma * 3.0).ceil() as i64;
    let mut value = [0.0_f64; 4];
    let mut total = 0.0;
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let weight = (-((dx * dx + dy * dy) as f64) / (2.0 * sigma * sigma)).exp();
            let sx = (i64::from(x) + dx).clamp(0, i64::from(extent.width() - 1)) as u32;
            let sy = (i64::from(y) + dy).clamp(0, i64::from(extent.height() - 1)) as u32;
            for (channel, input) in value.iter_mut().zip(grid_pixel(sx, sy).channels()) {
                *channel += f64::from(input) * weight;
            }
            total += weight;
        }
    }
    value.map(|channel| (channel / total) as f32)
}

#[test]
fn gaussian_regions_match_direct_reference_across_tiles_and_clamped_image_edges() -> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(259, 135)?;
        let (mut graph, source) = graph(context, extent)?;
        let blurred =
            graph.add(Operation::GaussianBlur { input: source, blur: GaussianBlur::new(1.25)? })?;
        let mut evaluator = Evaluator::new(graph, Grid { extent }, 16, context.memory())?;
        for region in
            [Region::new(0, 0, 3, 3)?, Region::new(126, 126, 5, 5)?, Region::new(257, 133, 2, 2)?]
        {
            let tile = evaluator.region(blurred, region, context)?;
            for y in region.y()..region.bottom() {
                for x in region.x()..region.right() {
                    let actual = tile
                        .pixel(x, y)
                        .ok_or(ImageError::Invalid("Blurred pixel missing"))?
                        .channels();
                    for (a, b) in actual.into_iter().zip(reference(x, y, extent, 1.25)) {
                        assert!((a - b).abs() < 2e-6, "{x},{y}: {actual:?}");
                    }
                }
            }
        }
        assert!(evaluator.stats().source_pixels < extent.pixels());
        Ok(())
    })
}

#[test]
fn gaussian_zero_aliases_pixels_and_sigma_changes_structural_identity() -> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(128, 128)?;
        let (mut graph, source) = graph(context, extent)?;
        let zero =
            graph.add(Operation::GaussianBlur { input: source, blur: GaussianBlur::new(0.0)? })?;
        let one =
            graph.add(Operation::GaussianBlur { input: source, blur: GaussianBlur::new(1.0)? })?;
        let other =
            graph.add(Operation::GaussianBlur { input: source, blur: GaussianBlur::new(2.0)? })?;
        assert_ne!(graph.fingerprint(one)?, graph.fingerprint(other)?);
        let mut evaluator = Evaluator::new(graph, Grid { extent }, 8, context.memory())?;
        let original = evaluator.tile(source, 0, 0, context)?;
        assert!(Arc::ptr_eq(&original, &evaluator.tile(zero, 0, 0, context)?));
        Ok(())
    })
}

#[test]
fn gaussian_premultiplied_color_survives_transparent_edges_without_dark_fringes() -> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(17, 17)?;
        let mut graph = Graph::new(8, context.memory())?;
        let shape = graph.add(Operation::Rectangle {
            extent,
            shape: VectorRectangle::new([8.0, 8.0, 9.0, 9.0], Pixel::linear(4.0, 1.0, 0.0, 1.0)?)?,
            translation: [0; 2],
        })?;
        let blurred =
            graph.add(Operation::GaussianBlur { input: shape, blur: GaussianBlur::new(1.0)? })?;
        let mut evaluator = Evaluator::new(graph, Grid { extent }, 8, context.memory())?;
        let tile = evaluator.tile(blurred, 0, 0, context)?;
        for pixel in tile.pixels() {
            let [red, green, blue, alpha] = pixel.channels();
            assert_eq!(blue, 0.0);
            assert!((red - 4.0 * alpha).abs() < 1e-6);
            assert!((green - alpha).abs() < 1e-6);
        }
        assert_eq!(tile.pixel(0, 0), Some(Pixel::TRANSPARENT));
        Ok(())
    })
}
