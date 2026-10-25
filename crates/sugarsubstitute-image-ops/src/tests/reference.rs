// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compare tiled separable execution against a direct two-dimensional reference filter.
use super::*;

fn weight(distance: f64) -> f64 {
    if distance.abs() < 1e-12 {
        1.0
    } else if distance.abs() >= 3.0 {
        0.0
    } else {
        let v = distance * std::f64::consts::PI;
        (v.sin() / v) * ((v / 3.0).sin() / (v / 3.0))
    }
}
fn reference(source: Extent, target: Extent, x: u32, y: u32) -> Result<Pixel, ImageError> {
    let ratio_x = f64::from(source.width()) / f64::from(target.width());
    let ratio_y = f64::from(source.height()) / f64::from(target.height());
    let scale_x = ratio_x.max(1.0);
    let scale_y = ratio_y.max(1.0);
    let cx = (f64::from(x) + 0.5) * ratio_x - 0.5;
    let cy = (f64::from(y) + 0.5) * ratio_y - 0.5;
    let mut result = [0.0_f64; 4];
    let mut total = 0.0;
    for iy in (cy - 3.0 * scale_y).ceil() as i64..=(cy + 3.0 * scale_y).floor() as i64 {
        for ix in (cx - 3.0 * scale_x).ceil() as i64..=(cx + 3.0 * scale_x).floor() as i64 {
            let w = weight((ix as f64 - cx) / scale_x) * weight((iy as f64 - cy) / scale_y);
            let pixel = grid_pixel(
                ix.clamp(0, i64::from(source.width()) - 1) as u32,
                iy.clamp(0, i64::from(source.height()) - 1) as u32,
            );
            for (result, channel) in result.iter_mut().zip(pixel.channels()) {
                *result += f64::from(channel) * w;
            }
            total += w;
        }
    }
    Pixel::filtered(result.map(|channel| (channel / total) as f32))
}

#[test]
fn tiled_lanczos_matches_reference_at_seams_edges_and_fractional_phases() -> TestResult {
    run(MEMORY, |context| {
        for (source, target) in [
            (Extent::new(513, 195)?, Extent::new(257, 111)?),
            (Extent::new(97, 81)?, Extent::new(259, 199)?),
        ] {
            let (mut graph, input) = graph(context, source)?;
            let output = graph.add(Operation::Resample { input, extent: target })?;
            let mut evaluator =
                Evaluator::new(graph, Grid { extent: source }, 24, context.memory())?;
            for ty in 0..target.height().div_ceil(TILE_EDGE) {
                for tx in 0..target.width().div_ceil(TILE_EDGE) {
                    let tile = evaluator.tile(output, tx, ty, context)?;
                    for y in tile.region().y()..tile.region().bottom() {
                        for x in tile.region().x()..tile.region().right() {
                            if x % 13 != 0
                                && y % 17 != 0
                                && x % TILE_EDGE > 1
                                && y % TILE_EDGE > 1
                                && x + 1 != target.width()
                                && y + 1 != target.height()
                            {
                                continue;
                            }
                            let actual = tile
                                .pixel(x, y)
                                .ok_or(ImageError::Invalid("Missing evaluated pixel"))?;
                            let expected = reference(source, target, x, y)?;
                            for (a, b) in actual.channels().into_iter().zip(expected.channels()) {
                                assert!((a - b).abs() < 2e-6, "Pixel {x},{y}: {a} versus {b}");
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
fn odd_pyramids_preserve_constant_color_and_normalized_edge_coverage() -> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(513, 257)?;
        let color = Pixel::linear(2.0, 0.4, -0.2, 0.5)?;
        let mut graph = Graph::new(32, context.memory())?;
        let source = graph.add(Operation::Solid { extent, color })?;
        let pyramid = graph.pyramid(source)?;
        let output = *pyramid.levels().last().ok_or(ImageError::Invalid("Empty pyramid"))?;
        let mut evaluator = Evaluator::new(graph, Grid { extent }, 48, context.memory())?;
        let tile = evaluator.tile(output, 0, 0, context)?;
        for (a, b) in tile.pixels()[0].channels().into_iter().zip(color.channels()) {
            assert!((a - b).abs() < 2e-6);
        }
        assert_eq!(evaluator.stats().source_pixels, 0);
        assert!(evaluator.stats().resident_tiles <= 48);
        Ok(())
    })
}
