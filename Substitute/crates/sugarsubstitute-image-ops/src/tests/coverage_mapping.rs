// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Convex mask boundaries and piecewise and bilinear coverage mappings, as shared edges use.
use super::*;

const FULL: u16 = FULL_COVERAGE;

fn filled(
    extent: Extent,
    regions: &[Region],
    context: &JobContext,
) -> Result<Coverage, ImageError> {
    let mut coverage = Coverage::new(extent, 0, context.memory())?;
    for region in regions {
        coverage.update(*region, context.memory(), |_, _, _| FULL)?;
    }
    Ok(coverage)
}

#[test]
fn convex_boundaries_follow_occupied_rows_and_drop_collinear_and_near_vertices() -> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(300, 200)?;
        let square = filled(extent, &[Region::new(10, 20, 100, 50)?], context)?;
        assert_eq!(
            square.convex_boundary(32_896),
            vec![[10.0, 20.0], [110.0, 20.0], [110.0, 70.0], [10.0, 70.0]]
        );
        // Half-covered pixels count from 128 of 255 on.
        let mut faint = square.clone();
        faint.update(Region::new(110, 20, 40, 50)?, context.memory(), |_, _, _| 32_895)?;
        assert_eq!(faint.convex_boundary(32_896), square.convex_boundary(32_896));
        // An L shape is framed by its hull; content kept beyond the extent is included.
        let shape =
            filled(extent, &[Region::new(0, 0, 40, 100)?, Region::new(0, 60, 120, 40)?], context)?;
        assert_eq!(
            shape.convex_boundary(1),
            vec![[0.0, 0.0], [40.0, 0.0], [120.0, 60.0], [120.0, 100.0], [0.0, 100.0]]
        );
        let away = square.translate([250, 0], context)?;
        assert_eq!(away.convex_boundary(1)[2], [360.0, 70.0]);
        assert!(Coverage::new(extent, 0, context.memory())?.convex_boundary(1).is_empty());
        Ok(())
    })
}

#[test]
fn piecewise_maps_triangulate_shared_ears_and_sample_the_nearest_source_pixel() -> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(200, 200)?;
        let source = [[0.0, 0.0], [0.0, 100.0], [100.0, 100.0], [100.0, 0.0]];
        // The right edge's lower corner slides right: a trapezoid.
        let target = [[0.0, 0.0], [0.0, 100.0], [150.0, 100.0], [100.0, 0.0]];
        let mapping = PiecewiseMapping::new(&source, &target)?;
        let back = mapping.inverse_map([125.0, 99.0]).ok_or(ImageError::Invalid("Outside"))?;
        assert!((0.0..=100.0).contains(&back[0]) && (back[1] - 99.0).abs() < 1.0, "{back:?}");
        assert!(mapping.inverse_map([180.0, 50.0]).is_none());
        let coverage = filled(extent, &[Region::new(0, 0, 100, 100)?], context)?;
        let mapped = coverage.remap(&CoverageMapping::Piecewise(mapping), context)?;
        assert_eq!(
            [mapped.value(140, 98), mapped.value(140, 20), mapped.value(50, 50)],
            [FULL, 0, FULL]
        );
        // Opposite windings, too few vertices and self-crossings are refused.
        let reversed: Vec<[f64; 2]> = target.iter().rev().copied().collect();
        assert!(PiecewiseMapping::new(&source, &reversed).is_err());
        assert!(PiecewiseMapping::new(&source[..3], &target[..3]).is_err());
        let crossed = [[0.0, 0.0], [100.0, 100.0], [0.0, 100.0], [100.0, 0.0]];
        assert!(PiecewiseMapping::new(&source, &crossed).is_err());
        Ok(())
    })
}

#[test]
fn bilinear_maps_carry_a_quad_onto_a_joined_triangle() -> TestResult {
    run(MEMORY, |context| {
        let extent = Extent::new(200, 200)?;
        let source = [[0.0, 0.0], [100.0, 0.0], [100.0, 100.0], [0.0, 100.0]];
        // The top edge collapses to its left corner.
        let target = [[0.0, 0.0], [0.0, 0.0], [100.0, 100.0], [0.0, 100.0]];
        let mapping = BilinearMapping::new(source, target)?;
        let point = mapping.inverse_map([50.0, 100.0]).ok_or(ImageError::Invalid("Outside"))?;
        assert!((point[0] - 50.0).abs() < 1e-9 && (point[1] - 100.0).abs() < 1e-9);
        let coverage = filled(extent, &[Region::new(0, 0, 100, 100)?], context)?;
        let mapped = coverage.remap(&CoverageMapping::Bilinear(mapping), context)?;
        assert_eq!([mapped.value(10, 90), mapped.value(90, 10)], [FULL, 0]);
        assert!(BilinearMapping::new(source, source).is_err());
        let affine = triangle_affine(
            [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
            [[5.0, 5.0], [7.0, 5.0], [5.0, 8.0]],
        )
        .ok_or(ImageError::Invalid("Degenerate"))?;
        assert_eq!(affine.map([1.0, 1.0]), [7.0, 8.0]);
        assert!(triangle_affine([[0.0, 0.0], [1.0, 1.0], [2.0, 2.0]], [[0.0; 2]; 3]).is_none());
        Ok(())
    })
}
