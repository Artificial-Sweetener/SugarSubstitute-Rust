// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve page focus at the actual mouse, pen and occluded-control routing boundary.

use super::*;

#[test]
fn captured_mouse_press_still_reaches_the_page_without_releasing_its_focus() -> TestResult {
    let mut routing = routing(&[])?;
    let message = press(Button::Primary, [50, 40]);
    let forwarded = routing.route(message);
    assert!(forwarded);
    assert!(!crate::main_actor::focus::releases_page_text_entry(true, forwarded));
    assert!(drain(&mut routing).contains(&button(MainCanvasButton::Primary, true)));
    Ok(())
}

#[test]
fn only_captured_pen_contact_start_releases_focus_and_page_controls_keep_it() -> TestResult {
    let field = PhysicalRect { x: 50, y: 30, width: 10, height: 10 };
    for (owner, point, expected_releases) in [
        ("canvas", [80, 50], 1),
        ("toolbar", [20, 25], 0),
        ("text field", [55, 35], 0),
        ("outside", [5, 5], 0),
    ] {
        let mut routing = routing(&[MENU, field])?;
        let mut releases = 0;
        for message in [
            moved(point, false),
            press(Button::Primary, point),
            moved(point, true),
            moved(point, true),
            released(Button::Primary, point, false),
        ] {
            let forwarded = routing.route_pen(message);
            releases += usize::from(crate::main_actor::focus::releases_page_text_entry(
                matches!(message, Message::Press { .. }),
                forwarded,
            ));
        }
        assert_eq!(releases, expected_releases, "{owner}");
    }
    Ok(())
}
