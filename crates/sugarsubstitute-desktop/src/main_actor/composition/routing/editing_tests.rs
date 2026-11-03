// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Editing tools' cursors over the region and modifier changes during a held gesture.

use super::*;

type TestResult = Result<(), Box<dyn std::error::Error>>;

const CLIP: PhysicalRect = PhysicalRect { x: 10, y: 20, width: 100, height: 50 };
const RULE: ClickRule = ClickRule { time: 1_000, bounds: [4, 4], limit: 500, drag: [4, 4] };
const SHIFT: ToolModifiers = ToolModifiers { shift: true, ctrl: false, alt: false };

fn sample(point: [f64; 2], modifiers: ToolModifiers) -> PointerSample {
    PointerSample::mouse(point, modifiers)
}

fn drain(routing: &mut Routing) -> Vec<MainCanvasGesture> {
    std::iter::from_fn(|| routing.take()).collect()
}

#[test]
fn an_editing_tool_cursor_wins_over_the_hands_inside_the_region_and_during_its_gesture()
-> TestResult {
    let mut routing = Routing::new()?;
    routing.publish(Some((CLIP, &[], false, Some(CursorIntent::Precise))));
    assert_eq!(routing.cursor(), None, "the pointer has not entered the region");
    routing.route(Message::Move { sample: sample([50.0, 40.0], ToolModifiers::NONE), held: false });
    assert_eq!(routing.cursor(), Some(CursorIntent::Precise));
    let press = sample([50.0, 40.0], ToolModifiers::NONE);
    routing.route(Message::Press { button: Button::Primary, sample: press, rule: RULE });
    routing.route(Message::Move { sample: sample([300.0, 40.0], ToolModifiers::NONE), held: true });
    assert_eq!(routing.cursor(), Some(CursorIntent::Precise), "a held gesture keeps it outside");
    routing.publish(Some((CLIP, &[], false, None)));
    assert_eq!(routing.cursor(), Some(CursorIntent::ClosedHand), "navigation's hands return");
    routing.publish(None);
    assert_eq!(routing.cursor(), None);
    Ok(())
}

#[test]
fn a_modifier_change_during_a_held_gesture_redelivers_the_latest_sample() -> TestResult {
    let mut routing = Routing::new()?;
    routing.publish(Some((CLIP, &[], false, None)));
    routing.route(Message::Move { sample: sample([50.0, 40.0], ToolModifiers::NONE), held: false });
    routing.refresh_modifiers(SHIFT);
    assert_eq!(drain(&mut routing).len(), 1, "hover alone re-delivers nothing");
    let press = sample([50.0, 40.0], ToolModifiers::NONE);
    routing.route(Message::Press { button: Button::Primary, sample: press, rule: RULE });
    routing.route(Message::Move { sample: sample([60.0, 45.0], ToolModifiers::NONE), held: true });
    drain(&mut routing);
    routing.refresh_modifiers(ToolModifiers::NONE);
    assert_eq!(drain(&mut routing), [], "unchanged modifiers re-deliver nothing");
    routing.refresh_modifiers(SHIFT);
    routing.refresh_modifiers(SHIFT);
    assert_eq!(drain(&mut routing), [MainCanvasGesture::Pointer(sample([60.0, 45.0], SHIFT))]);
    let release = sample([60.0, 45.0], SHIFT);
    routing.route(Message::Release { button: Button::Primary, sample: release, held: false });
    drain(&mut routing);
    routing.refresh_modifiers(ToolModifiers::NONE);
    assert_eq!(drain(&mut routing), [], "nothing is held after the release");
    Ok(())
}
