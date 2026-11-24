// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate shared caption artwork, tinting, placement, the console's destination glyph and
//! scale rejection.

use super::{CaptionIcon, CaptionIcons};
use sugarsubstitute_ui::{bitmap::Bitmap, shared_icons::SharedIcon};

/// Paint one glyph at one scale into a fresh caption-row raster.
fn painted(
    icons: &mut CaptionIcons,
    scale: f64,
    kind: CaptionIcon,
    tint: [u8; 3],
) -> Result<Bitmap, Box<dyn std::error::Error>> {
    let mut target = Bitmap::new((558.0 * scale).ceil() as u32, (32.0 * scale).ceil() as u32)
        .ok_or("allocation")?;
    icons.draw(&mut target, scale, kind, tint)?;
    Ok(target)
}

/// Every glyph paints only inside its own button and keeps the requested tint at every scale.
#[test]
fn glyphs_stay_in_their_buttons_in_their_tint() -> Result<(), Box<dyn std::error::Error>> {
    let mut icons = CaptionIcons::new();
    for scale in [1.0, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0, 3.5, 4.0] {
        for (kind, left, right) in [
            (CaptionIcon::Console { open: false }, 466.0, 512.0),
            (CaptionIcon::Console { open: true }, 466.0, 512.0),
            (CaptionIcon::Close, 512.0, 558.0),
        ] {
            for tint in [[255; 3], [0; 3]] {
                let target = painted(&mut icons, scale, kind, tint)?;
                let (from, to) = ((left * scale).floor() as u32, (right * scale).ceil() as u32);
                let mut inked = false;
                for y in 0..target.height() {
                    for x in 0..target.width() {
                        let pixel = target.pixel(x, y).ok_or("pixel")?;
                        if pixel.alpha() == 0 {
                            continue;
                        }
                        inked = true;
                        assert!((from..to).contains(&x), "{kind:?} at {scale}: x {x}");
                        let expected = tint.map(|channel| {
                            sugarsubstitute_ui::shapes::byte_product(channel, pixel.alpha())
                        });
                        assert_eq!([pixel.red(), pixel.green(), pixel.blue()], expected);
                    }
                }
                assert!(inked, "{kind:?} at {scale}");
            }
        }
    }
    Ok(())
}

/// The console glyph shows where the toggle goes: the solid bar while closed, the hollow bar
/// while open, and the two differ on screen.
#[test]
fn the_console_glyph_shows_its_destination() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(CaptionIcon::Console { open: false }.shared(), SharedIcon::ConsoleOpen);
    assert_eq!(CaptionIcon::Console { open: true }.shared(), SharedIcon::ConsoleClose);
    let mut icons = CaptionIcons::new();
    let closed = painted(&mut icons, 2.0, CaptionIcon::Console { open: false }, [255; 3])?;
    let open = painted(&mut icons, 2.0, CaptionIcon::Console { open: true }, [255; 3])?;
    let coverage =
        |bitmap: &Bitmap| bitmap.pixels().iter().map(|p| u32::from(p.alpha())).sum::<u32>();
    assert!(coverage(&closed) > coverage(&open), "the solid bar covers more than the hollow one");
    Ok(())
}

/// Reject invalid input without painting and support unlisted device scales.
#[test]
fn arbitrary_scales_remain_bounded_and_invalid_input_is_atomic()
-> Result<(), Box<dyn std::error::Error>> {
    let mut icons = CaptionIcons::new();
    let mut target = Bitmap::new(1228, 71).ok_or("allocation")?;
    for scale in [f64::NAN, f64::INFINITY, 0.0, 4.1] {
        assert!(icons.draw(&mut target, scale, CaptionIcon::Close, [255; 3]).is_err());
        assert!(target.pixels().iter().all(|p| p.alpha() == 0));
    }
    icons.draw(&mut target, 2.2, CaptionIcon::Close, [255; 3])?;
    assert!(target.pixels().iter().any(|p| p.alpha() > 0));
    assert!(
        (0..target.height())
            .all(|y| (0..1126).all(|x| target.pixel(x, y).is_some_and(|p| p.alpha() == 0)))
    );
    Ok(())
}
