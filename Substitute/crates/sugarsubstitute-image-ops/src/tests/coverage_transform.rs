// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Affine coverage maps and fragment placement against dense references.
use super::*;

const FULL: u16 = FULL_COVERAGE;

fn values(coverage: &Coverage) -> Result<Vec<u16>, ImageError> {
    let mut values = vec![0; coverage.extent().pixels() as usize];
    coverage.read(coverage.extent().region(), &mut values)?;
    Ok(values)
}

/// Irregular coverage across tile boundaries inside `[left, top, right, bottom]`.
fn pattern(
    extent: Extent,
    [left, top, right, bottom]: [u32; 4],
    context: &JobContext,
) -> Result<Coverage, ImageError> {
    let mut coverage = Coverage::new(extent, 0, context.memory())?;
    let region = Region::new(left, top, right - left, bottom - top)?;
    coverage
        .update(region, context.memory(), |x, y, _| ((x * 977 + y * 613) % 65_521 + 1) as u16)?;
    Ok(coverage)
}

fn rectangle(
    extent: Extent,
    [left, top, right, bottom]: [u32; 4],
    context: &JobContext,
) -> Result<Coverage, ImageError> {
    let mut coverage = Coverage::new(extent, 0, context.memory())?;
    coverage.update(
        Region::new(left, top, right - left, bottom - top)?,
        context.memory(),
        |_, _, _| FULL,
    )?;
    Ok(coverage)
}

/// A quarter turn about `[x, y]`, unaligned: its cosine is not exactly zero.
fn quarter(clockwise: bool, [x, y]: [f64; 2]) -> Result<CoverageAffine, ImageError> {
    let angle = if clockwise { 90.0_f64 } else { -90.0 }.to_radians();
    let (sin, cos) = angle.sin_cos();
    CoverageAffine::new([cos, sin, -sin, cos, x - (cos * x - sin * y), y - (sin * x + cos * y)])
}

#[test]
fn quarter_turns_and_flips_permute_values_exactly_and_round_trip() -> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(300, 260)?;
        // An odd-by-even frame: its centre is half a pixel off the lattice for quarter turns.
        let bounds = [41_u32, 37, 171, 200];
        let coverage = pattern(extent, bounds, context)?;
        let centre = [f64::from(41 + 171) / 2.0, f64::from(37 + 200) / 2.0];
        let right = quarter(true, centre)?.aligned();
        let left = quarter(false, centre)?.aligned();
        assert_eq!(right.matrix()[..4], [0.0, 1.0, -1.0, 0.0]);
        let turned = coverage.transform(right, context)?;
        // Every value survives exactly: the turn only permutes pixels.
        let mut before = values(&coverage)?;
        let mut after = values(&turned)?;
        before.sort_unstable();
        after.sort_unstable();
        assert_eq!(before, after);
        let back = turned.transform(left, context)?;
        assert_eq!(back.fingerprint(), coverage.fingerprint());
        let four =
            (0..4).try_fold(coverage.clone(), |current, _| current.transform(right, context))?;
        assert_eq!(four.fingerprint(), coverage.fingerprint());
        let flip = CoverageAffine::new([-1.0, 0.0, 0.0, 1.0, centre[0] * 2.0, 0.0])?.aligned();
        let flipped = coverage.transform(flip, context)?;
        assert_eq!(flipped.value(171 - 1, 50), coverage.value(41, 50));
        assert_eq!(flipped.transform(flip, context)?.fingerprint(), coverage.fingerprint());
        let vertical = CoverageAffine::new([1.0, 0.0, 0.0, -1.0, 0.0, centre[1] * 2.0])?.aligned();
        let flopped = coverage.transform(vertical, context)?;
        assert_eq!(flopped.value(60, 200 - 1), coverage.value(60, 37));
        Ok(())
    })
}

#[test]
fn general_maps_sample_bilinearly_and_whole_pixel_moves_match_translation() -> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(160, 140)?;
        let coverage = pattern(extent, [20, 30, 90, 100], context)?;
        let moved = coverage.transform(CoverageAffine::translation([7.0, -4.0]), context)?;
        assert_eq!(moved.fingerprint(), coverage.translate([7, -4], context)?.fingerprint());
        // Half a pixel to the right averages each pair of horizontal neighbours.
        let half = coverage.transform(CoverageAffine::translation([0.5, 0.0]), context)?;
        let source = values(&coverage)?;
        let at = |x: i64, y: i64| {
            if x < 0 || y < 0 || x >= 160 || y >= 140 {
                0.0
            } else {
                f64::from(source[y as usize * 160 + x as usize])
            }
        };
        for (x, y) in [(20_i64, 30_i64), (21, 31), (55, 64), (90, 99), (91, 50)] {
            let expected = ((at(x - 1, y) + at(x, y)) / 2.0).round() as u16;
            assert_eq!(half.value(x as u32, y as u32), expected, "{x},{y}");
        }
        // A scale by two around the origin reads every source pixel twice.
        let double = CoverageAffine::new([2.0, 0.0, 0.0, 2.0, 0.0, 0.0])?;
        let scaled = rectangle(extent, [10, 10, 30, 20], context)?.transform(double, context)?;
        assert_eq!(
            [scaled.value(25, 25), scaled.value(55, 35), scaled.value(70, 50)],
            [FULL, FULL, 0]
        );
        assert!(CoverageAffine::new([1.0, 2.0, 0.5, 1.0, 0.0, 0.0]).is_err());
        assert!(CoverageAffine::new([f64::NAN, 0.0, 0.0, 1.0, 0.0, 0.0]).is_err());
        let affine = CoverageAffine::new([0.8, 0.3, -0.2, 1.1, 5.0, -3.0])?;
        let inverse = affine.inverse().ok_or(ImageError::Invalid("No inverse"))?;
        let [x, y] = affine.then(inverse).map([13.0, 21.0]);
        assert!((x - 13.0).abs() < 1e-9 && (y - 21.0).abs() < 1e-9);
        Ok(())
    })
}

#[test]
fn placing_a_fragment_cuts_its_alpha_and_adds_its_premultiplied_values() -> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(200, 180)?;
        let mask = rectangle(extent, [0, 0, 100, 100], context)?;
        let selection = rectangle(extent, [40, 40, 140, 140], context)?;
        let fragment = mask.combine(&selection, CombineMode::Intersect, context)?;
        let hole = mask.combine(&selection, CombineMode::Subtract, context)?;
        let (fragment, alpha) =
            (fragment.translate([50, 30], context)?, selection.translate([50, 30], context)?);
        let placed = hole.over(&fragment, &alpha, context)?;
        // The lifted square leaves its hole and lands 50 right and 30 down, replacing what
        // lay under the moved selection.
        assert_eq!(placed.value(10, 10), FULL);
        assert_eq!(placed.value(60, 60), 0);
        assert_eq!(placed.value(120, 100), FULL);
        assert_eq!(placed.value(160, 160), 0);
        assert_eq!(placed.value(95, 75), FULL);
        let soft = Coverage::new(extent, FULL / 2, context.memory())?;
        let empty = Coverage::new(extent, 0, context.memory())?;
        let halved = mask.over(&empty, &soft, context)?;
        assert_eq!(halved.value(10, 10), FULL / 2 + 1);
        assert!(
            mask.over(&empty, &Coverage::new(Extent::new(5, 5)?, 0, context.memory())?, context)
                .is_err()
        );
        Ok(())
    })
}

#[test]
fn occupied_bounds_follow_the_requested_threshold() -> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(300, 300)?;
        let mut coverage = Coverage::new(extent, 0, context.memory())?;
        coverage.update(Region::new(10, 10, 200, 200)?, context.memory(), |x, _, _| {
            if x < 50 { 100 } else { FULL }
        })?;
        assert_eq!(coverage.bounds(), Some(Region::new(10, 10, 200, 200)?));
        assert_eq!(coverage.bounds_at_least(128), Some(Region::new(50, 10, 160, 200)?));
        assert_eq!(coverage.bounds_at_least(0), Some(extent.region()));
        assert_eq!(Coverage::new(extent, 0, context.memory())?.bounds_at_least(1), None);
        Ok(())
    })
}
