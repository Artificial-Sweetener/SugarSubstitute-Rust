// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Qualify original asset identity, lazy retention, and malformed input at the character owner.

use super::{CharacterError, CharacterLibrary};
use crate::animation::AnimationFrame;
use crate::assets::POSE_ASSETS;
use crate::projection::Bounds;
use std::error::Error;
use tiny_skia::Pixmap;

/// Allocate a test target through the same fallible API used by the renderer.
fn target() -> Result<Pixmap, Box<dyn Error>> {
    Pixmap::new(386, 386).ok_or_else(|| "Cannot allocate test raster".into())
}

/// Keep the untransformed premultiplied pixels identical to the original PNG.
#[test]
fn bootstrap_preserves_identity_pixels_and_decodes_one_pose() -> Result<(), Box<dyn Error>> {
    let mut library = CharacterLibrary::new(POSE_ASSETS)?;
    assert_eq!(library.decoded_count(), 0);
    let mut target = target()?;
    let bounds = Bounds { x: 0.0, y: 0.0, width: 386.0, height: 386.0 };
    library.render(&mut target, bounds, AnimationFrame::bootstrap())?;
    let source = Pixmap::decode_png(POSE_ASSETS[0].bytes)?;
    assert_eq!(target.data(), source.data());
    assert_eq!(library.decoded_count(), 1);
    library.render(&mut target, bounds, AnimationFrame::bootstrap())?;
    assert_eq!(library.decoded_count(), 1);
    Ok(())
}

/// Match Qt's snapped bootstrap position in the original odd-width mascot slot.
#[test]
fn front_facing_odd_width_slot_snaps_without_blurring() -> Result<(), Box<dyn Error>> {
    let mut library = CharacterLibrary::new(POSE_ASSETS)?;
    let mut target = Pixmap::new(558, 558).ok_or("Cannot allocate test raster")?;
    library.render(
        &mut target,
        Bounds { x: 83.0, y: 7.0, width: 387.0, height: 386.0 },
        AnimationFrame::bootstrap(),
    )?;
    let source = Pixmap::decode_png(POSE_ASSETS[0].bytes)?;
    for row in 0..386 {
        let start = ((row + 7) * 558 + 84) * 4;
        assert_eq!(
            &target.data()[start..start + 386 * 4],
            &source.data()[row * 386 * 4..(row + 1) * 386 * 4]
        );
    }
    Ok(())
}

/// Validate every original resource while retaining each only when first requested.
#[test]
fn all_packaged_poses_decode_lazily_at_source_size() -> Result<(), Box<dyn Error>> {
    let mut library = CharacterLibrary::new(POSE_ASSETS)?;
    let mut target = target()?;
    let bounds = Bounds { x: 0.0, y: 0.0, width: 386.0, height: 386.0 };
    for pose in 0..40 {
        library.render(
            &mut target,
            bounds,
            AnimationFrame { pose, ..AnimationFrame::bootstrap() },
        )?;
        assert_eq!(library.decoded_count(), pose + 1);
    }
    Ok(())
}

/// Preserve valid alpha and widget clipping for every pose across both flip directions.
#[test]
fn all_poses_remain_bounded_through_sheared_edge_and_landing_frames() -> Result<(), Box<dyn Error>>
{
    let mut library = CharacterLibrary::new(POSE_ASSETS)?;
    let mut target = Pixmap::new(209, 209).ok_or("Cannot allocate test raster")?;
    let bounds = Bounds { x: 8.0, y: 8.0, width: 193.0, height: 193.0 };
    for pose in 0..40 {
        for direction in [-1, 1] {
            for progress in [0.25, 0.5, 0.875] {
                target.fill(tiny_skia::Color::TRANSPARENT);
                library.render(
                    &mut target,
                    bounds,
                    AnimationFrame { pose, progress, direction },
                )?;
                assert!(target.pixels().iter().any(|pixel| pixel.alpha() > 0));
                assert!(target.pixels().iter().all(|pixel| {
                    pixel.red() <= pixel.alpha()
                        && pixel.green() <= pixel.alpha()
                        && pixel.blue() <= pixel.alpha()
                }));
                for y in 0..209 {
                    for x in 0..209 {
                        if !(8..201).contains(&x) || !(8..201).contains(&y) {
                            assert_eq!(target.pixel(x, y).map(|pixel| pixel.alpha()), Some(0));
                        }
                    }
                }
            }
        }
    }
    assert_eq!(library.decoded_count(), 40);
    Ok(())
}

/// Reject invalid geometry and pose indices before changing a target or decoding assets.
#[test]
fn malformed_frames_do_not_mutate_raster() -> Result<(), Box<dyn Error>> {
    let mut library = CharacterLibrary::new(POSE_ASSETS)?;
    let mut target = target()?;
    let empty = target.data().to_vec();
    let bounds = Bounds { x: 0.0, y: 0.0, width: 386.0, height: 386.0 };
    assert!(matches!(
        library.render(&mut target, Bounds { width: 0.0, ..bounds }, AnimationFrame::bootstrap()),
        Err(CharacterError::InvalidBounds)
    ));
    assert!(matches!(
        library.render(
            &mut target,
            bounds,
            AnimationFrame { pose: 40, ..AnimationFrame::bootstrap() }
        ),
        Err(CharacterError::PoseIndex { index: 40 })
    ));
    assert_eq!(library.decoded_count(), 0);
    assert_eq!(target.data(), empty);
    Ok(())
}
