// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Cursor artwork geometry, hotspots and DPI scaling.

use super::*;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn pixel(image: &CursorImage, x: u32, y: u32) -> [u8; 4] {
    let index = ((y * image.width() + x) * 4) as usize;
    let rgba = image.rgba();
    [rgba[index], rgba[index + 1], rgba[index + 2], rgba[index + 3]]
}

/// Antialiased edges may blend a channel by a few levels.
fn assert_near(actual: [u8; 4], expected: [u8; 4]) {
    assert!(
        actual.iter().zip(expected).all(|(a, e)| a.abs_diff(e) <= 16),
        "{actual:?} vs {expected:?}"
    );
}

fn opaque(image: &CursorImage) -> usize {
    image.rgba().chunks_exact(4).filter(|pixel| pixel[3] > 0).count()
}

#[test]
fn brush_ring_layers_black_white_and_colour_around_a_transparent_centre() -> TestResult {
    let art = CursorArt::BrushOutline { diameter: 40, color: [255, 0, 0], erase: false };
    let image = art.render(1.0)?;
    assert_eq!([image.width(), image.height()], [48, 48]);
    assert_eq!(image.hotspot(), [24, 24]);
    // The ring's centre line is the top row of a 39 pixel circle centred at 24.
    assert_near(pixel(&image, 24, 4), [255, 0, 0, 255]);
    assert_near(pixel(&image, 24, 3), [255, 255, 255, 255]);
    assert_near(pixel(&image, 24, 2), [0, 0, 0, 255]);
    assert_eq!(pixel(&image, 24, 0)[3], 0);
    assert_eq!(pixel(&image, 24, 24)[3], 0);
    assert_eq!(pixel(&image, 0, 0)[3], 0);
    Ok(())
}

#[test]
fn brush_ring_keeps_its_device_diameter_and_scales_its_pens_and_padding() -> TestResult {
    let art = CursorArt::BrushOutline { diameter: 80, color: [0, 128, 255], erase: false };
    let image = art.render(2.0)?;
    assert_eq!([image.width(), image.height()], [96, 96]);
    assert_eq!(image.hotspot(), [48, 48]);
    let ring_top = 8 + 1;
    assert_near(pixel(&image, 48, ring_top), [0, 128, 255, 255]);
    assert_near(pixel(&image, 48, ring_top - 2), [255, 255, 255, 255]);
    assert_near(pixel(&image, 48, ring_top - 4), [0, 0, 0, 255]);
    Ok(())
}

#[test]
fn the_erase_mark_adds_a_lower_right_underscore_only_when_requested() -> TestResult {
    let plain =
        CursorArt::BrushOutline { diameter: 100, color: [9, 9, 9], erase: false }.render(1.0)?;
    let erase =
        CursorArt::BrushOutline { diameter: 100, color: [9, 9, 9], erase: true }.render(1.0)?;
    assert!(opaque(&erase) > opaque(&plain));
    let changed: Vec<[u32; 2]> = (0..erase.height())
        .flat_map(|y| (0..erase.width()).map(move |x| [x, y]))
        .filter(|[x, y]| pixel(&erase, *x, *y) != pixel(&plain, *x, *y))
        .collect();
    assert!(changed.iter().all(|[x, y]| *x > 50 && *y > 50), "the mark stays lower right");
    assert!(changed.iter().any(|[x, y]| pixel(&erase, *x, *y) == [255, 255, 255, 255]));
    Ok(())
}

#[test]
fn precise_crosshair_leaves_its_hotspot_clear_between_outlined_arms() -> TestResult {
    let image = CursorArt::Precise(PreciseMark::None).render(1.0)?;
    assert_eq!([image.width(), image.height()], [40, 40]);
    assert_eq!(image.hotspot(), [20, 20]);
    assert_eq!(pixel(&image, 20, 20)[3], 0, "the gap keeps the target visible");
    assert_near(pixel(&image, 19, 10), [0, 0, 0, 255]);
    assert_near(pixel(&image, 18, 10), [255, 255, 255, 255]);
    assert_eq!(pixel(&image, 16, 10)[3], 0);
    assert_near(pixel(&image, 10, 19), [0, 0, 0, 255]);
    let doubled = CursorArt::Precise(PreciseMark::None).render(2.0)?;
    assert_eq!([doubled.width(), doubled.hotspot()[0]], [80, 40]);
    Ok(())
}

#[test]
fn precise_add_and_subtract_marks_differ_from_the_plain_crosshair() -> TestResult {
    let plain = CursorArt::Precise(PreciseMark::None).render(1.0)?;
    let add = CursorArt::Precise(PreciseMark::Add).render(1.0)?;
    let subtract = CursorArt::Precise(PreciseMark::Subtract).render(1.0)?;
    assert_eq!(pixel(&add, 31, 31), [0, 0, 0, 255], "the plus centre is black");
    assert_eq!(pixel(&plain, 31, 31)[3], 0);
    assert!(opaque(&subtract) > opaque(&plain));
    assert_ne!(add, subtract);
    Ok(())
}

#[test]
fn fluent_artwork_renders_at_forty_logical_pixels_with_a_scaled_hotspot() -> TestResult {
    for art in [CursorArt::SelectionTranslate, CursorArt::MoveCut] {
        let base = art.render(1.0)?;
        assert_eq!(
            [base.width(), base.height(), base.hotspot()[0], base.hotspot()[1]],
            [40, 40, 6, 3]
        );
        assert!(pixel(&base, 7, 5)[3] > 0, "{art:?} draws its arrow at the hotspot");
        let large = art.render(1.5)?;
        assert_eq!([large.width(), large.hotspot()[0], large.hotspot()[1]], [60, 9, 5]);
        assert!(opaque(&large) > opaque(&base));
    }
    assert_ne!(CursorArt::SelectionTranslate.render(1.0)?, CursorArt::MoveCut.render(1.0)?);
    Ok(())
}

#[test]
fn unsupported_scales_and_ring_sizes_are_rejected() {
    let ring = |diameter| CursorArt::BrushOutline { diameter, color: [0; 3], erase: false };
    assert!(ring(1).render(1.0).is_err());
    assert!(ring(CursorArt::MAX_RING_DIAMETER + 1).render(1.0).is_err());
    assert!(ring(CursorArt::MAX_RING_DIAMETER).render(1.0).is_ok());
    assert!(CursorArt::MoveCut.render(0.0).is_err());
    assert!(CursorArt::Precise(PreciseMark::Add).render(f32::NAN).is_err());
}

#[test]
fn transform_arrows_and_rotation_arcs_turn_in_whole_degrees_about_a_centred_hotspot() -> TestResult
{
    let level = CursorArt::Affine { rotation: false, degrees: 0 }.render(1.0)?;
    assert_eq!([level.width(), level.height(), level.hotspot()[0]], [32, 32, 16]);
    // The light shaft, outlined dark, runs through the centre between the tips at 6 and 26.
    let shaft = pixel(&level, 16, 16);
    assert!(shaft[3] > 240 && shaft[0] > 150, "{shaft:?}");
    let outline = pixel(&level, 16, 14);
    assert!(outline[3] > 150 && outline[0] < 40, "{outline:?}");
    assert!(pixel(&level, 7, 16)[3] > 0 && pixel(&level, 25, 16)[3] > 0);
    assert_eq!(pixel(&level, 16, 6)[3], 0);
    let upright = CursorArt::Affine { rotation: false, degrees: 90 }.render(1.0)?;
    assert!(pixel(&upright, 16, 7)[3] > 0 && pixel(&upright, 7, 16)[3] == 0);
    let rotation = CursorArt::Affine { rotation: true, degrees: 0 }.render(2.0)?;
    assert_eq!([rotation.width(), rotation.hotspot()[0]], [64, 32]);
    // The arc's top lies six pixels above the centre; the centre itself stays clear.
    assert!(pixel(&rotation, 32, 20)[3] > 0);
    assert_eq!(pixel(&rotation, 32, 32)[3], 0);
    let turned = CursorArt::Affine { rotation: true, degrees: 90 }.render(2.0)?;
    assert_ne!(turned.rgba(), rotation.rgba());
    Ok(())
}
