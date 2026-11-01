// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Display sampling keeps coverage, edge clamping and fixed-point interpolation exact.

use super::*;

#[test]
fn display_samples_distinguish_coverage_nearest_and_centered_bilinear_positions() {
    for position in [-1.0, 3.0, f64::NAN, f64::INFINITY] {
        assert_eq!(DisplaySample::from_corner(position, 3, true), DisplaySample::UNCOVERED);
    }
    assert_eq!(DisplaySample::from_corner(0.0, 0, true), DisplaySample::UNCOVERED);
    let nearest = DisplaySample::from_corner(1.9, 3, false);
    assert_eq!((nearest.first(), nearest.second(), nearest.weight()), (1, 1, 0));
    assert!(nearest.is_covered() && nearest.is_valid_for(3));

    let middle = DisplaySample::from_corner(1.0, 3, true);
    assert_eq!((middle.first(), middle.second(), middle.weight()), (0, 1, 128));
    let first = DisplaySample::from_corner(0.0, 3, true);
    assert_eq!((first.first(), first.second(), first.weight()), (0, 1, 0));
    let last = DisplaySample::from_corner(2.99, 3, true);
    assert_eq!((last.first(), last.second(), last.weight()), (2, 2, 0));
    assert!(DisplaySample::UNCOVERED.is_valid_for(0));
}

#[test]
fn bilinear_encoded_pixels_use_one_q16_rounding_step_and_remain_opaque() {
    let half = DisplaySample::from_corner(1.0, 3, true);
    let pixel =
        bilinear_display_pixel([0xff00_0000, 0xff40_4040, 0xff80_8080, 0xffff_ffff], half, half);
    assert_eq!(pixel, 0xff70_7070);

    let exact = DisplaySample::from_corner(0.5, 3, true);
    assert_eq!(bilinear_display_pixel([0xff12_3456, 0, 0, 0], exact, exact), 0xff12_3456);
}

#[test]
fn display_rows_match_per_pixel_interpolation_exactly() {
    let width = 37_u32;
    let pixel = |seed: u32| 0xff00_0000 | (seed.wrapping_mul(2_654_435_761) >> 8);
    let top: Vec<u32> = (0..width).map(|x| pixel(x + 1)).collect();
    let bottom: Vec<u32> = (0..width).map(|x| pixel(x + 101)).collect();
    let matte = 0xff05_0505;
    let mut columns: Vec<DisplaySample> = (0..96)
        .map(|x| DisplaySample::from_corner(f64::from(x) * 0.397 + 0.11, width, true))
        .collect();
    columns.extend([
        DisplaySample::UNCOVERED,
        DisplaySample::from_corner(36.9, width, true),
        DisplaySample::from_corner(12.4, width, false),
    ]);
    for vertical in [
        DisplaySample::from_corner(0.5, 2, true),
        DisplaySample::from_corner(1.0, 2, true),
        DisplaySample::from_corner(1.37, 2, true),
        DisplaySample::from_corner(0.2, 2, false),
    ] {
        let mut row = vec![0; columns.len()];
        bilinear_display_row(&top, &bottom, vertical, &columns, matte, &mut row);
        for (x, column) in columns.iter().enumerate() {
            let expected = if !column.is_covered() {
                matte
            } else if column.weight() == 0 && vertical.weight() == 0 {
                top[column.first() as usize]
            } else {
                let corners = [
                    top[column.first() as usize],
                    top[column.second() as usize],
                    bottom[column.first() as usize],
                    bottom[column.second() as usize],
                ];
                bilinear_display_pixel(corners, *column, vertical)
            };
            assert_eq!(row[x], expected, "column {x} with vertical weight {}", vertical.weight());
        }
    }
}

#[test]
fn display_rows_preserve_exact_source_pixels_without_interpolation() {
    let top = [0x1234_5678_u32, 0x9abc_def0];
    let nearest = DisplaySample::from_corner(1.2, 2, false);
    let mut row = [0; 1];
    bilinear_display_row(
        &top,
        &top,
        DisplaySample::from_corner(0.5, 1, false),
        &[nearest],
        0,
        &mut row,
    );
    assert_eq!(row, [0x9abc_def0]);
}
