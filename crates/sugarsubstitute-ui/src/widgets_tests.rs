// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify form-state colors, alpha composition, clipping and preparation bounds.
use super::*;
use std::error::Error;

fn draw(decoration: Decoration) -> Result<Bitmap, Box<dyn Error>> {
    let mut target = Bitmap::new(100, 60).ok_or("Target absent")?;
    let mut cursor = DecorationCursor::new(
        100,
        60,
        Bounds { x: 8.0, y: 8.0, width: 80.0, height: 32.0 },
        decoration,
        1.0,
    )?;
    while !cursor.step(&mut target)? {}
    Ok(target)
}

#[test]
fn primary_action_and_disabled_state_keep_the_original_palette() -> Result<(), Box<dyn Error>> {
    for (state, expected) in [
        (ControlState::Rest, [255, 64, 137, 255]),
        (ControlState::Hover, [243, 58, 128, 255]),
        (ControlState::Pressed, [243, 58, 128, 255]),
        (ControlState::Disabled, [52, 52, 52, 255]),
    ] {
        let target = draw(Decoration::Button { primary: true, state })?;
        let pixel = target.pixel(40, 20).ok_or("Button interior absent")?;
        assert_eq!([pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()], expected);
        assert_eq!(target.pixel(7, 20).ok_or("Clip pixel absent")?.alpha(), 0);
    }
    Ok(())
}

#[test]
fn focused_field_uses_two_accent_rows_and_preserves_the_other_edges() -> Result<(), Box<dyn Error>>
{
    let target =
        draw(Decoration::Field { selection: false, focused: true, state: ControlState::Rest })?;
    for y in [38, 39] {
        let pixel = target.pixel(40, y).ok_or("Field edge absent")?;
        assert_eq!([pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()], [255, 64, 137, 255]);
    }
    assert_eq!(target.pixel(40, 37).ok_or("Field interior absent")?.alpha(), 10);
    assert!(target.pixel(8, 20).ok_or("Field side absent")?.alpha() < 255);
    Ok(())
}

#[test]
fn form_cursor_rejects_a_changed_target_before_any_mutation() -> Result<(), Box<dyn Error>> {
    let b = Bounds { x: 8.0, y: 8.0, width: 80.0, height: 32.0 };
    let mut cursor = DecorationCursor::new(100, 60, b, Decoration::Section, 1.0)?;
    let mut wrong = Bitmap::new(100, 61).ok_or("Target absent")?;
    assert!(cursor.step(&mut wrong).is_err());
    assert!(wrong.data().iter().all(|v| *v == 0));
    for width in [0.0, f32::NAN, f32::INFINITY, 4097.0] {
        assert!(
            DecorationCursor::new(100, 60, Bounds { width, ..b }, Decoration::Section, 1.0)
                .is_err()
        );
    }
    Ok(())
}

#[test]
fn fractional_page_placement_renders_at_installer_qualification_scales()
-> Result<(), Box<dyn Error>> {
    for scale in [1.0, 1.25, 1.5, 2.0] {
        let width = (1180.0 * scale) as u32;
        let height = (760.0 * scale) as u32;
        let mut target = Bitmap::new(width, height).ok_or("Target absent")?;
        let mut cursor = DecorationCursor::new(
            width,
            height,
            Bounds { x: 380.0, y: 442.5, width: 420.0, height: 33.0 },
            Decoration::Field { selection: true, focused: false, state: ControlState::Rest },
            scale,
        )?;
        loop {
            let stage = match &cursor.stage {
                Stage::Outer(_) => "outer",
                Stage::Inner(_) => "inner",
                Stage::Paint => "paint",
                Stage::Done => "done",
            };
            let result = cursor.step(&mut target);
            assert!(result.is_ok(), "scale {scale}, stage {stage}: {result:?}");
            if result? {
                break;
            }
        }
        assert!(
            target
                .pixel((400.0 * scale) as u32, (450.0 * scale) as u32)
                .is_some_and(|p| p.alpha() == 15)
        );
    }
    Ok(())
}

/// Match the saved dark installer checkbox independently of its containing translucent section.
#[test]
fn checkbox_resting_pixels_match_the_installer_baseline() -> Result<(), Box<dyn Error>> {
    let target = draw(Decoration::Check { checked: false })?;
    for (x, expected) in [(40, 59), (8, 133)] {
        let pixel = target.pixel(x, 20).ok_or("Checkbox pixel absent")?;
        assert_eq!(
            [pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()],
            [expected, expected, expected, 255]
        );
    }
    Ok(())
}
