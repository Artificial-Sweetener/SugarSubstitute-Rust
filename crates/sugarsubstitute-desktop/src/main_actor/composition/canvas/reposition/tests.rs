// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Check where retained pixels show at changed geometry without a compositor.

use super::*;
use crate::canvas::placement::PhysicalRect;

fn placement(viewport: [i32; 4], clip: [i32; 4]) -> CanvasPlacement {
    let rect = |[x, y, width, height]: [i32; 4]| PhysicalRect {
        x,
        y,
        width: width.unsigned_abs(),
        height: height.unsigned_abs(),
    };
    CanvasPlacement { viewport: rect(viewport), clip: rect(clip) }
}

fn rect(left: f32, top: f32, right: f32, bottom: f32) -> D2D_RECT_F {
    D2D_RECT_F { left, top, right, bottom }
}

#[test]
fn grown_viewport_centres_the_retained_pixels_inside_its_clip()
-> Result<(), Box<dyn std::error::Error>> {
    let shown = placement([100, 50, 600, 400], [100, 50, 600, 400]);
    let centred = centred(shown, Extent::new(400, 300)?);
    assert_eq!(centred.offset, [200, 100]);
    assert_eq!(centred.clip, rect(0.0, 0.0, 400.0, 300.0));
    Ok(())
}

#[test]
fn shrunk_viewport_crops_the_retained_pixels_to_its_clip() -> Result<(), Box<dyn std::error::Error>>
{
    let shown = placement([100, 50, 200, 100], [120, 50, 180, 100]);
    let centred = centred(shown, Extent::new(400, 300)?);
    assert_eq!(centred.offset, [0, -50]);
    assert_eq!(centred.clip, rect(120.0, 100.0, 300.0, 200.0));
    Ok(())
}

#[test]
fn moved_viewport_of_the_same_size_shows_every_retained_pixel_at_its_new_origin()
-> Result<(), Box<dyn std::error::Error>> {
    let shown = placement([-30, 10, 400, 300], [0, 10, 370, 300]);
    let centred = centred(shown, Extent::new(400, 300)?);
    assert_eq!(centred.offset, [-30, 10]);
    assert_eq!(centred.clip, rect(30.0, 0.0, 400.0, 300.0));
    Ok(())
}

#[test]
fn clip_outside_the_retained_pixels_shows_none_of_them() -> Result<(), Box<dyn std::error::Error>> {
    let shown = placement([0, 0, 1000, 1000], [0, 0, 100, 100]);
    let centred = centred(shown, Extent::new(200, 200)?);
    assert_eq!(centred.offset, [400, 400]);
    assert_eq!(centred.clip, rect(0.0, 0.0, 0.0, 0.0));
    Ok(())
}
