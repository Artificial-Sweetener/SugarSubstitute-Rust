// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify form-state colors, alpha composition, clipping and preparation bounds.
use super::*;
use std::error::Error;

fn draw(decoration: Decoration) -> Result<Bitmap, Box<dyn Error>> {
    draw_with(decoration, AccentPalette::PRODUCT)
}

fn draw_with(decoration: Decoration, accent: AccentPalette) -> Result<Bitmap, Box<dyn Error>> {
    draw_mode(decoration, Theme::Dark, accent)
}

fn draw_mode(
    decoration: Decoration,
    theme: Theme,
    accent: AccentPalette,
) -> Result<Bitmap, Box<dyn Error>> {
    let mut target = Bitmap::new(100, 60).ok_or("Target absent")?;
    let mut cursor = DecorationCursor::new(
        100,
        60,
        Bounds { x: 8.0, y: 8.0, width: 80.0, height: 32.0 },
        decoration,
        theme,
        accent,
        1.0,
    )?;
    while !cursor.step(&mut target)? {}
    Ok(target)
}

/// Actual decoration pixels use independent Qt-derived light/dark vectors in every enabled state.
#[test]
fn resolved_mode_reaches_primary_button_pixels_and_foreground() -> Result<(), Box<dyn Error>> {
    for (theme, colors, foreground) in [
        (Theme::Dark, [[255, 68, 132, 255], [230, 62, 119, 255], [209, 60, 110, 255]], 0),
        (Theme::Light, [[233, 30, 99, 255], [245, 32, 104, 255], [245, 106, 153, 255]], 255),
    ] {
        for ((state, alpha), expected) in
            [(ControlState::Rest, 255), (ControlState::Hover, 255), (ControlState::Pressed, 161)]
                .into_iter()
                .zip(colors)
        {
            let bitmap = draw_mode(
                Decoration::Button { primary: true, state },
                theme,
                AccentPalette::PRODUCT,
            )?;
            let pixel = bitmap.pixel(40, 20).ok_or("Primary interior absent")?;
            assert_eq!([pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()], expected);
            assert_eq!(
                TextTone::Action { primary: true, state }.color(theme),
                [foreground, foreground, foreground, alpha]
            );
        }
        let text = if theme == Theme::Dark { 255 } else { 0 };
        assert_eq!(TextTone::Primary.color(theme), [text, text, text, 255]);
        assert_eq!(TextTone::Secondary.color(theme), [text, text, text, 189]);
        let bitmap = draw_mode(Decoration::Section, theme, AccentPalette::PRODUCT)?;
        let pixel = bitmap.pixel(40, 20).ok_or("Section interior absent")?;
        assert_eq!(
            [pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()],
            if theme == Theme::Dark { [10, 10, 10, 10] } else { [0, 0, 0, 10] }
        );
    }
    Ok(())
}

#[test]
fn primary_action_and_disabled_state_use_the_qfluentwidgets_palette() -> Result<(), Box<dyn Error>>
{
    for (state, expected) in [
        (ControlState::Rest, [255, 68, 132, 255]),
        (ControlState::Hover, [230, 62, 119, 255]),
        (ControlState::Pressed, [209, 60, 110, 255]),
        (ControlState::Disabled, [52, 52, 52, 255]),
    ] {
        let target = draw(Decoration::Button { primary: true, state })?;
        let pixel = target.pixel(40, 20).ok_or("Button interior absent")?;
        assert_eq!([pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()], expected);
        assert_eq!(target.pixel(7, 20).ok_or("Clip pixel absent")?.alpha(), 0);
    }
    Ok(())
}

/// Every accent role follows the surface's supplied system accent rather than product pink.
#[test]
fn emphasized_controls_paint_the_supplied_system_accent() -> Result<(), Box<dyn Error>> {
    let accent =
        AccentPalette::system(sugarsubstitute_contracts::appearance::AccentColor::new(0, 120, 212));
    for (decoration, expected) in [
        (Decoration::Button { primary: true, state: ControlState::Rest }, [41, 162, 255, 255]),
        (Decoration::Button { primary: true, state: ControlState::Hover }, [37, 146, 230, 255]),
        (Decoration::Check { checked: true, state: ControlState::Rest }, [41, 162, 255, 255]),
    ] {
        let target = draw_with(decoration, accent)?;
        let pixel = target.pixel(40, 20).ok_or("Control interior absent")?;
        assert_eq!([pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()], expected);
    }
    let field = draw_with(
        Decoration::Field { selection: false, focused: true, state: ControlState::Rest },
        accent,
    )?;
    let edge = field.pixel(40, 39).ok_or("Field edge absent")?;
    assert_eq!([edge.red(), edge.green(), edge.blue(), edge.alpha()], [41, 162, 255, 255]);
    Ok(())
}

/// Borders, checkbox states and pressed text follow distinct qfluentwidgets roles.
#[test]
fn accent_states_reach_widget_borders_checks_and_foregrounds() {
    let accent =
        AccentPalette::system(sugarsubstitute_contracts::appearance::AccentColor::new(0, 120, 212));
    for (state, fill, border, bottom) in [
        (ControlState::Rest, [41, 162, 255, 255], [58, 169, 255, 255], [88, 182, 255, 255]),
        (ControlState::Hover, [37, 146, 230, 255], [58, 169, 255, 255], [88, 182, 255, 255]),
        (ControlState::Pressed, [37, 135, 209, 255], [37, 135, 209, 255], [37, 135, 209, 255]),
    ] {
        let palette = Decoration::Button { primary: true, state }.palette(Theme::Dark, accent);
        assert_eq!(palette.fill, fill);
        assert_eq!(palette.border, border);
        assert_eq!(palette.bottom, bottom);
        let checkbox = Decoration::Check { checked: true, state }.palette(Theme::Dark, accent);
        assert_eq!(checkbox.fill, fill);
        assert_eq!(checkbox.border, fill);
    }
    let disabled = Decoration::Check { checked: true, state: ControlState::Disabled }
        .palette(Theme::Dark, accent);
    assert_eq!(disabled.fill, [255, 255, 255, 41]);
    assert_eq!(disabled.border[3], 0);
    assert_eq!(
        TextTone::Action { primary: true, state: ControlState::Pressed }.color(Theme::Dark),
        [0, 0, 0, 161]
    );
    assert_eq!(
        TextTone::Action { primary: true, state: ControlState::Rest }.color(Theme::Dark),
        [0, 0, 0, 255]
    );
}

#[test]
fn focused_field_uses_two_accent_rows_and_preserves_the_other_edges() -> Result<(), Box<dyn Error>>
{
    let target =
        draw(Decoration::Field { selection: false, focused: true, state: ControlState::Rest })?;
    for y in [38, 39] {
        let pixel = target.pixel(40, y).ok_or("Field edge absent")?;
        assert_eq!([pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()], [255, 68, 132, 255]);
    }
    assert_eq!(target.pixel(40, 37).ok_or("Field interior absent")?.alpha(), 10);
    assert!(target.pixel(8, 20).ok_or("Field side absent")?.alpha() < 255);
    Ok(())
}

#[test]
fn form_cursor_rejects_a_changed_target_before_any_mutation() -> Result<(), Box<dyn Error>> {
    let b = Bounds { x: 8.0, y: 8.0, width: 80.0, height: 32.0 };
    let mut cursor = DecorationCursor::new(
        100,
        60,
        b,
        Decoration::Section,
        Theme::Dark,
        AccentPalette::PRODUCT,
        1.0,
    )?;
    let mut wrong = Bitmap::new(100, 61).ok_or("Target absent")?;
    assert!(cursor.step(&mut wrong).is_err());
    assert!(wrong.data().iter().all(|v| *v == 0));
    for width in [0.0, f32::NAN, f32::INFINITY, 4097.0] {
        assert!(
            DecorationCursor::new(
                100,
                60,
                Bounds { width, ..b },
                Decoration::Section,
                Theme::Dark,
                AccentPalette::PRODUCT,
                1.0
            )
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
            Theme::Dark,
            AccentPalette::PRODUCT,
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
    let target = draw(Decoration::Check { checked: false, state: ControlState::Rest })?;
    for (x, expected) in [(40, 59), (8, 133)] {
        let pixel = target.pixel(x, 20).ok_or("Checkbox pixel absent")?;
        assert_eq!(
            [pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()],
            [expected, expected, expected, 255]
        );
    }
    Ok(())
}
