// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Coverage storage, rasterization, operations and graph evaluation.
use super::*;

const FULL: u16 = FULL_COVERAGE;

fn values(coverage: &Coverage) -> Result<Vec<u16>, ImageError> {
    let mut values = vec![0; coverage.extent().pixels() as usize];
    coverage.read(coverage.extent().region(), &mut values)?;
    Ok(values)
}

/// Dense reference with explicit zero padding, written for clarity rather than speed.
fn reference_morphology(
    source: &[u16],
    width: usize,
    height: usize,
    radius: usize,
    grow: bool,
) -> Vec<u16> {
    let pass = |input: &[u16], vertical: bool| {
        let mut output = vec![0; input.len()];
        for y in 0..height {
            for x in 0..width {
                let mut extreme = if grow { 0 } else { FULL };
                for offset in -(radius as i64)..=radius as i64 {
                    let (sx, sy) = if vertical {
                        (x as i64, y as i64 + offset)
                    } else {
                        (x as i64 + offset, y as i64)
                    };
                    let value = if sx < 0 || sy < 0 || sx >= width as i64 || sy >= height as i64 {
                        0
                    } else {
                        input[sy as usize * width + sx as usize]
                    };
                    extreme = if grow { extreme.max(value) } else { extreme.min(value) };
                }
                output[y * width + x] = extreme;
            }
        }
        output
    };
    pass(&pass(source, false), true)
}

fn reference_feather(source: &[u16], width: usize, height: usize, radius: f32) -> Vec<u16> {
    let boxed = ((radius * 0.577_35).round() as i64).max(1);
    let mut values: Vec<f32> = source.iter().map(|value| f32::from(*value)).collect();
    for _ in 0..3 {
        for vertical in [false, true] {
            let input = values.clone();
            for y in 0..height {
                for x in 0..width {
                    let mut sum = 0.0;
                    for offset in -boxed..=boxed {
                        let (sx, sy) = if vertical {
                            (x as i64, y as i64 + offset)
                        } else {
                            (x as i64 + offset, y as i64)
                        };
                        if sx >= 0 && sy >= 0 && sx < width as i64 && sy < height as i64 {
                            sum += input[sy as usize * width + sx as usize];
                        }
                    }
                    values[y * width + x] = sum / (2 * boxed + 1) as f32;
                }
            }
        }
    }
    values.iter().map(|value| value.round().clamp(0.0, 65535.0) as u16).collect()
}

#[test]
fn coverage_shares_tiles_between_clones_and_identifies_equal_content() -> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(300, 200)?;
        let empty = Coverage::new(extent, 0, context.memory())?;
        assert_eq!(empty.uniform(), Some(0));
        assert_eq!(empty.bounds(), None);
        assert_eq!(empty.stored_bytes(), 0);
        let mut painted = empty.clone();
        let region = Region::new(120, 100, 20, 40)?;
        painted.update(region, context.memory(), |x, y, _| ((x + y) % 7 + 1) as u16 * 1000)?;
        assert_eq!(empty.value(125, 110), 0, "edits never reach shared clones");
        assert_eq!(painted.value(125, 110), 5000);
        assert_eq!(painted.bounds(), Some(region));
        assert_ne!(painted.fingerprint(), empty.fingerprint());
        // The edit crosses two tile columns and two tile rows: four tiles are stored.
        assert_eq!(painted.stored_bytes(), 4 * 128 * 128 * 2);
        let mut read = vec![0; region.pixels() as usize];
        painted.read(region, &mut read)?;
        let mut copy = Coverage::new(extent, 0, context.memory())?;
        copy.write(region, &read, context.memory())?;
        assert_eq!(copy, painted, "equal values give equal identity");
        copy.update(region, context.memory(), |_, _, _| 0)?;
        assert_eq!(copy, empty, "tiles returning to a single value collapse");
        assert_eq!(copy.stored_bytes(), 0);
        assert!(painted.read(Region::new(290, 0, 20, 1)?, &mut [0; 20]).is_err());
        Ok(())
    })
}

#[test]
fn paths_rasterize_exact_area_with_nonzero_winding_and_extent_clipping() -> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(260, 140)?;
        let aligned = Path::rectangle([10.0, 20.0, 30.0, 25.0])?.rasterize(extent, context)?;
        assert_eq!(aligned.bounds(), Some(Region::new(10, 20, 20, 5)?));
        assert_eq!(aligned.value(10, 20), FULL);
        assert_eq!(aligned.value(29, 24), FULL);
        let half = Path::rectangle([10.5, 20.0, 30.0, 20.5])?.rasterize(extent, context)?;
        assert_eq!(half.value(10, 20), FULL / 4 + 1, "a quarter-covered pixel");
        assert_eq!(half.value(11, 20), FULL / 2 + 1);
        // Partly outside the extent, and crossing a tile boundary.
        let clipped = Path::rectangle([-40.0, -5.0, 129.5, 3.0])?.rasterize(extent, context)?;
        assert_eq!(clipped.value(0, 0), FULL);
        assert_eq!(clipped.value(128, 2), FULL);
        assert_eq!(clipped.value(129, 2), FULL / 2 + 1);
        assert_eq!(clipped.value(0, 3), 0);
        // Overlapping contours never exceed full coverage.
        let twice = Path::polygon(vec![
            [50.0, 50.0],
            [60.0, 50.0],
            [60.0, 60.0],
            [50.0, 60.0],
            [50.0, 50.0],
            [60.0, 50.0],
            [60.0, 60.0],
            [50.0, 60.0],
        ])?
        .rasterize(extent, context)?;
        assert_eq!(twice.value(55, 55), FULL);
        // A sloped edge splits each crossed pixel by area: the triangle's area is preserved.
        let triangle = Path::polygon(vec![[140.0, 10.0], [240.0, 10.0], [140.0, 110.0]])?
            .rasterize(extent, context)?;
        let sum: f64 =
            values(&triangle)?.iter().map(|value| f64::from(*value) / f64::from(FULL)).sum();
        assert!((sum - 5000.0).abs() < 0.05, "triangle area {sum}");
        let ellipse = Path::ellipse([20.0, 40.0, 120.0, 120.0])?.rasterize(extent, context)?;
        let sum: f64 =
            values(&ellipse)?.iter().map(|value| f64::from(*value) / f64::from(FULL)).sum();
        let expected = std::f64::consts::PI * 50.0 * 40.0;
        assert!((sum - expected).abs() < 1.0, "ellipse area {sum} against {expected}");
        assert!(Path::polygon(vec![[0.0, 0.0], [1.0, f64::NAN], [2.0, 2.0]]).is_err());
        assert!(Path::rectangle([5.0, 5.0, 5.0, 9.0]).is_err());
        Ok(())
    })
}

#[test]
fn combination_follows_coverage_compositing() -> TestResult {
    let half = FULL / 2 + 1;
    assert_eq!(CombineMode::Add.apply(half, half), 49_152);
    assert_eq!(CombineMode::Subtract.apply(FULL, half), FULL - half);
    assert_eq!(CombineMode::Intersect.apply(half, FULL), half);
    assert_eq!(CombineMode::Replace.apply(FULL, 3), 3);
    run(MEMORY, |context| {
        let extent = Extent::new(200, 200)?;
        let left = Path::rectangle([0.0, 0.0, 100.0, 200.0])?.rasterize(extent, context)?;
        let top = Path::rectangle([0.0, 0.0, 200.0, 100.0])?.rasterize(extent, context)?;
        let union = left.combine(&top, CombineMode::Add, context)?;
        assert_eq!(
            [union.value(150, 50), union.value(50, 150), union.value(150, 150)],
            [FULL, FULL, 0]
        );
        let cut = left.combine(&top, CombineMode::Subtract, context)?;
        assert_eq!([cut.value(50, 50), cut.value(50, 150)], [0, FULL]);
        let both = left.combine(&top, CombineMode::Intersect, context)?;
        assert_eq!(both.bounds(), Some(Region::new(0, 0, 100, 100)?));
        assert_eq!(left.combine(&top, CombineMode::Replace, context)?, top);
        let inverted = left.invert(context.memory())?;
        assert_eq!([inverted.value(50, 50), inverted.value(150, 50)], [0, FULL]);
        assert_eq!(inverted.invert(context.memory())?, left);
        let other = Coverage::new(Extent::new(10, 10)?, 0, context.memory())?;
        assert!(left.combine(&other, CombineMode::Add, context).is_err());
        Ok(())
    })
}

#[test]
fn morphology_and_feather_match_dense_reference_filters() -> TestResult {
    run(MEMORY, |context| {
        let (width, height) = (190_usize, 150_usize);
        let extent = Extent::new(width as u32, height as u32)?;
        let mut coverage = Coverage::new(extent, 0, context.memory())?;
        // Soft, irregular coverage touching the right edge and crossing tile boundaries.
        coverage.update(Region::new(100, 60, 90, 80)?, context.memory(), |x, y, _| {
            if (x * 7 + y * 3) % 11 < 6 { ((x * 911 + y * 577) % 65_536) as u16 } else { 0 }
        })?;
        let source = values(&coverage)?;
        for radius in [1, 4, 17] {
            assert_eq!(
                values(&coverage.expand(radius, context)?)?,
                reference_morphology(&source, width, height, radius as usize, true),
                "expand {radius}"
            );
            assert_eq!(
                values(&coverage.contract(radius, context)?)?,
                reference_morphology(&source, width, height, radius as usize, false),
                "contract {radius}"
            );
        }
        for radius in [0.4_f32, 4.0, 9.0] {
            let feathered = values(&coverage.feather(radius, context)?)?;
            let reference = reference_feather(&source, width, height, radius);
            let worst =
                feathered.iter().zip(&reference).map(|(a, b)| a.abs_diff(*b)).max().unwrap_or(0);
            assert!(worst <= 1, "feather {radius} differs by {worst}");
        }
        assert_eq!(coverage.feather(0.0, context)?, coverage);
        assert!(coverage.feather(-1.0, context).is_err());
        // Contracting a select-all treats the extent edge as uncovered.
        let all = Coverage::new(extent, FULL, context.memory())?.contract(2, context)?;
        assert_eq!([all.value(1, 50), all.value(2, 50), all.value(187, 50)], [0, FULL, FULL]);
        assert_eq!(all.value(188, 50), 0);
        Ok(())
    })
}

#[test]
fn coverage8_round_trips_every_value_exactly() -> TestResult {
    for value in 0..=255_u8 {
        assert_eq!(coverage_to_u8(coverage_from_u8(value)), value);
    }
    assert_eq!(coverage_to_u8(FULL / 2), 127);
    run(MEMORY, |context| {
        let extent = Extent::new(131, 129)?;
        let bytes: Vec<u8> = (0..extent.pixels()).map(|index| (index * 37 % 256) as u8).collect();
        let coverage = Coverage::from_coverage8(extent, &bytes, context)?;
        assert_eq!(coverage.to_coverage8(context)?, bytes);
        assert!(Coverage::from_coverage8(extent, &bytes[1..], context).is_err());
        Ok(())
    })
}

#[test]
fn graph_coverage_masks_images_and_tracks_coverage_identity() -> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(140, 40)?;
        let coverage = Path::rectangle([0.0, 0.0, 70.0, 40.0])?.rasterize(extent, context)?;
        let (mut graph, source) = super::graph(context, extent)?;
        let mask = graph.coverage(&coverage)?;
        let masked = graph.add(Operation::Mask { input: source, mask })?;
        let mut edited = coverage.clone();
        edited.update(Region::new(100, 0, 1, 1)?, context.memory(), |_, _, _| FULL)?;
        let other = graph.coverage(&edited)?;
        assert_ne!(graph.fingerprint(mask)?, graph.fingerprint(other)?);
        let Operation::Coverage { source: retained, .. } = graph.operation(mask)? else {
            return Err(ImageError::Invalid("Coverage node changed operation"));
        };
        assert_eq!(graph.coverage_source(retained)?, &coverage);
        let mut evaluator = Evaluator::new(graph, Grid { extent }, 16, context.memory())?;
        let tile = evaluator.tile(masked, 0, 0, context)?;
        assert_eq!(tile.pixel(10, 10), Some(grid_pixel(10, 10)));
        assert_eq!(tile.pixel(100, 10), Some(Pixel::TRANSPARENT));
        let white = evaluator.tile(mask, 1, 0, context)?;
        assert_eq!(white.pixel(130, 0), Some(Pixel::TRANSPARENT));
        assert_eq!(evaluator.tile(other, 0, 0, context)?.pixel(100, 0), Some(Pixel([1.0; 4])));
        Ok(())
    })
}

#[test]
fn nearest_previews_read_only_the_coverage_samples_they_show() -> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(4000, 3000)?;
        let coverage = Path::ellipse([100.0, 200.0, 3900.0, 2800.0])?.rasterize(extent, context)?;
        let mut graph = Graph::new(8, context.memory())?;
        let source = graph.coverage(&coverage)?;
        let preview = graph.resize_with_filter(source, Extent::new(100, 75)?, Filter::Nearest)?;
        let Operation::Resample { sampling, .. } = graph.operation(preview)? else {
            return Err(ImageError::Invalid("Preview lost its resample"));
        };
        assert!(graph.sampled_coverage(preview, Region::new(0, 0, 100, 75)?)?.is_some());
        let mut evaluator = Evaluator::new(graph, Grid { extent }, 4, context.memory())?;
        let tile = evaluator.tile(preview, 0, 0, context)?;
        let samples = RasterSamples::new(extent, tile.region(), sampling)?;
        for (row, y) in samples.rows().iter().enumerate() {
            for (column, x) in samples.columns().iter().enumerate() {
                let alpha = f32::from(coverage.value(*x, *y)) / f32::from(FULL);
                assert_eq!(tile.pixel(column as u32, row as u32), Some(Pixel([alpha; 4])));
            }
        }
        assert_eq!(evaluator.stats().source_pixels, 0);
        Ok(())
    })
}

#[test]
fn translation_moves_whole_pixels_and_drops_what_leaves_the_extent() -> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(300, 200)?;
        let square = Path::rectangle([10.0, 20.0, 50.0, 60.0])?.rasterize(extent, context)?;
        let moved = square.translate([130, -5], context)?;
        assert_eq!(moved.bounds(), Some(Region::new(140, 15, 40, 40)?));
        assert_eq!(moved.translate([-130, 5], context)?, square);
        let clipped = square.translate([-30, 0], context)?;
        assert_eq!(clipped.bounds(), Some(Region::new(0, 20, 20, 40)?));
        assert_eq!(square.translate([0, 400], context)?.bounds(), None);
        let everything = Coverage::new(extent, FULL, context.memory())?;
        let shifted = everything.translate([5, 7], context)?;
        assert_eq!(
            [shifted.value(4, 100), shifted.value(5, 7), shifted.value(299, 199)],
            [0, FULL, FULL]
        );
        assert_eq!(square.translate([0, 0], context)?, square);
        Ok(())
    })
}

/// Fingerprints are persisted identities, so their function of the values must never change.
#[test]
fn coverage_fingerprints_stay_a_fixed_function_of_extent_and_values() -> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(300, 200)?;
        let mut coverage = Coverage::new(extent, 0, context.memory())?;
        let region = Region::new(100, 60, 160, 120)?;
        coverage.update(region, context.memory(), |x, y, _| {
            (x.wrapping_mul(2_654_435_761) ^ y.wrapping_mul(40_503)) as u16
        })?;
        coverage.update(Region::new(0, 128, 128, 72)?, context.memory(), |_, _, _| 777)?;
        let hex: String = coverage.fingerprint().iter().map(|byte| format!("{byte:02x}")).collect();
        assert_eq!(hex, "d51176ecd96faa8228969399bbbbfce5a28f7d35fbb88edf6b3744b90c062133");
        Ok(())
    })
}
