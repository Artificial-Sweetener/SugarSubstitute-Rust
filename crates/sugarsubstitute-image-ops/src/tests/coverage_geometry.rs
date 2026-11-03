// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Coverage placement and resampling against dense references.
use super::*;

const FULL: u16 = FULL_COVERAGE;

fn values(coverage: &Coverage) -> Result<Vec<u16>, ImageError> {
    let mut values = vec![0; coverage.extent().pixels() as usize];
    coverage.read(coverage.extent().region(), &mut values)?;
    Ok(values)
}

/// Irregular coverage that crosses tile boundaries and leaves some tiles empty.
fn pattern(extent: Extent, context: &JobContext) -> Result<Coverage, ImageError> {
    let mut coverage = Coverage::new(extent, 0, context.memory())?;
    coverage.update(extent.region(), context.memory(), |x, y, _| {
        if x > extent.width() / 2 && y > extent.height() / 2 {
            0
        } else {
            ((x * 977 + y * 613) % 65_536) as u16
        }
    })?;
    Ok(coverage)
}

/// Pillow-style triangle filter written for clarity: widened by the reduction factor.
fn reference_smooth(source: &[u16], from: (usize, usize), to: (usize, usize)) -> Vec<u16> {
    let weights = |source: usize, target: usize, index: usize| {
        let ratio = source as f64 / target as f64;
        let support = ratio.max(1.0);
        let center = (index as f64 + 0.5) * ratio;
        let mut taps: Vec<(usize, f64)> = (0..source)
            .map(|tap| (tap, (1.0 - ((tap as f64 + 0.5 - center) / support).abs()).max(0.0)))
            .filter(|(_, weight)| *weight > 0.0)
            .collect();
        if taps.is_empty() {
            taps.push((((center.floor()) as usize).min(source - 1), 1.0));
        }
        let total: f64 = taps.iter().map(|(_, weight)| weight).sum();
        taps.into_iter().map(|(tap, weight)| (tap, weight / total)).collect::<Vec<_>>()
    };
    let mut output = vec![0; to.0 * to.1];
    for y in 0..to.1 {
        let rows = weights(from.1, to.1, y);
        for x in 0..to.0 {
            let columns = weights(from.0, to.0, x);
            let mut sum = 0.0;
            for (row, row_weight) in &rows {
                for (column, column_weight) in &columns {
                    sum += row_weight * column_weight * f64::from(source[row * from.0 + column]);
                }
            }
            output[y * to.0 + x] = sum.round().clamp(0.0, f64::from(FULL)) as u16;
        }
    }
    output
}

#[test]
fn reframing_places_coverage_at_an_offset_and_crops_what_falls_outside() -> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(200, 150)?;
        let coverage = pattern(extent, context)?;
        let source = values(&coverage)?;
        for (target, offset) in [
            (Extent::new(300, 260)?, [50_i64, 55]),
            (Extent::new(120, 90)?, [-40, -30]),
            (Extent::new(200, 150)?, [0, 0]),
            (Extent::new(260, 100)?, [-10, 70]),
        ] {
            let placed = coverage.reframe(target, offset, context.memory(), context)?;
            assert_eq!(placed.extent(), target);
            let actual = values(&placed)?;
            for y in 0..target.height() as i64 {
                for x in 0..target.width() as i64 {
                    let (sx, sy) = (x - offset[0], y - offset[1]);
                    let expected = if (0..200).contains(&sx) && (0..150).contains(&sy) {
                        source[sy as usize * 200 + sx as usize]
                    } else {
                        0
                    };
                    assert_eq!(actual[y as usize * target.width() as usize + x as usize], expected);
                }
            }
        }
        assert_eq!(coverage.reframe(extent, [0, 0], context.memory(), context)?, coverage);
        let gone = coverage.reframe(extent, [500, 0], context.memory(), context)?;
        assert_eq!(gone.uniform(), Some(0));
        assert_eq!(gone.stored_bytes(), 0, "uncovered target tiles store nothing");
        // A select-all only covers the area it is placed over.
        let all = Coverage::new(Extent::new(10, 10)?, FULL, context.memory())?;
        let padded = all.reframe(Extent::new(20, 10)?, [5, 0], context.memory(), context)?;
        assert_eq!([padded.value(4, 0), padded.value(5, 0), padded.value(14, 9)], [0, FULL, FULL]);
        assert_eq!(padded.value(15, 0), 0);
        Ok(())
    })
}

#[test]
fn nearest_resampling_takes_the_source_pixel_under_each_center() -> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(150, 140)?;
        let coverage = pattern(extent, context)?;
        for target in [Extent::new(300, 280)?, Extent::new(61, 33)?, Extent::new(150, 140)?] {
            let resampled = coverage.resample(
                target,
                CoverageResampling::Nearest,
                context.memory(),
                context,
            )?;
            for y in 0..target.height() {
                let sy = ((f64::from(y) + 0.5) * 140.0 / f64::from(target.height())) as u32;
                for x in 0..target.width() {
                    let sx = ((f64::from(x) + 0.5) * 150.0 / f64::from(target.width())) as u32;
                    assert_eq!(resampled.value(x, y), coverage.value(sx, sy));
                }
            }
        }
        // Doubling repeats every pixel, so halving again restores the original exactly.
        let doubled = coverage.resample(
            Extent::new(300, 280)?,
            CoverageResampling::Nearest,
            context.memory(),
            context,
        )?;
        let restored =
            doubled.resample(extent, CoverageResampling::Nearest, context.memory(), context)?;
        assert_eq!(restored, coverage);
        Ok(())
    })
}

#[test]
fn smooth_resampling_matches_a_dense_triangle_filter_and_keeps_uniform_coverage() -> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(170, 133)?;
        let coverage = pattern(extent, context)?;
        let source = values(&coverage)?;
        for target in [Extent::new(253, 301)?, Extent::new(41, 29)?, Extent::new(170, 66)?] {
            let resampled =
                coverage.resample(target, CoverageResampling::Smooth, context.memory(), context)?;
            let reference = reference_smooth(
                &source,
                (170, 133),
                (target.width() as usize, target.height() as usize),
            );
            let worst = values(&resampled)?
                .iter()
                .zip(&reference)
                .map(|(actual, expected)| actual.abs_diff(*expected))
                .max()
                .unwrap_or(0);
            assert!(worst <= 2, "smooth {target:?} differs by {worst}");
        }
        let half = Coverage::new(extent, FULL / 2, context.memory())?;
        let scaled = half.resample(
            Extent::new(400, 20)?,
            CoverageResampling::Smooth,
            context.memory(),
            context,
        )?;
        assert_eq!(scaled.uniform(), Some(FULL / 2));
        let empty = Coverage::new(extent, 0, context.memory())?;
        let enlarged = empty.resample(
            Extent::new(4000, 4000)?,
            CoverageResampling::Smooth,
            context.memory(),
            context,
        )?;
        assert_eq!((enlarged.uniform(), enlarged.stored_bytes()), (Some(0), 0));
        Ok(())
    })
}
