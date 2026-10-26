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
