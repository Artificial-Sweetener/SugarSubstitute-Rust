// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the composed-window routing table without a window, browser or canvas.

use super::*;
use MainCanvasGesture::{Cancel, DoubleClick, DragOut, Pointer, Wheel};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const CLIP: PhysicalRect = PhysicalRect { x: 10, y: 20, width: 100, height: 50 };
const MENU: PhysicalRect = PhysicalRect { x: 10, y: 20, width: 30, height: 10 };
const RULE: ClickRule = ClickRule { time: 1_000, bounds: [4, 4], limit: 500, drag: [4, 4] };

fn routing(occlusions: &[PhysicalRect]) -> Result<Routing, crate::DesktopError> {
    let mut routing = Routing::new()?;
    routing.publish(Some((CLIP, occlusions, false)));
    Ok(routing)
}

fn drain(routing: &mut Routing) -> Vec<MainCanvasGesture> {
    std::iter::from_fn(|| routing.take()).collect()
}

fn press(button: Button, point: [i32; 2]) -> Message {
    Message::Press { button, point, rule: RULE }
}

fn button(button: MainCanvasButton, pressed: bool) -> MainCanvasGesture {
    MainCanvasGesture::Button { button, pressed }
}

#[test]
fn without_a_region_every_message_belongs_to_webview2_only() -> TestResult {
    let mut routing = Routing::new()?;
    for message in [
        Message::Move { point: [50, 40], held: false },
        press(Button::Primary, [50, 40]),
        Message::Release { button: Button::Primary, point: [50, 40], held: false },
        Message::Wheel { point: [50, 40], delta: 120 },
        Message::Leave,
    ] {
        assert!(routing.route(message), "{message:?}");
    }
    assert_eq!(drain(&mut routing), []);
    assert_eq!(routing.cursor(), None);
    Ok(())
}

#[test]
fn inside_the_region_moves_reach_both_and_wheel_reaches_only_the_canvas() -> TestResult {
    let mut routing = routing(&[])?;
    assert!(routing.route(Message::Move { point: [50, 40], held: false }));
    assert!(!routing.route(Message::Wheel { point: [51, 41], delta: -60 }));
    assert_eq!(drain(&mut routing), [Pointer([51.0, 41.0]), Wheel(-0.5)]);
    assert_eq!(routing.cursor(), Some(CursorIntent::OpenHand));
    assert!(routing.route(Message::Wheel { point: [5, 5], delta: 120 }), "outside the clip");
    assert!(routing.route(Message::Other), "horizontal wheel stays with the page");
    assert_eq!(drain(&mut routing), []);
    Ok(())
}

#[test]
fn occlusions_keep_their_input_and_cursor_with_webview2() -> TestResult {
    let mut routing = routing(&[MENU])?;
    assert!(routing.route(Message::Move { point: [15, 25], held: false }));
    assert_eq!(routing.cursor(), None);
    assert!(routing.route(press(Button::Primary, [15, 25])));
    assert!(routing.route(Message::Wheel { point: [15, 25], delta: 120 }));
    assert_eq!(drain(&mut routing), []);
    assert!(routing.route(Message::Release {
        button: Button::Primary,
        point: [15, 25],
        held: false
    }));
    assert!(routing.route(Message::Move { point: [60, 40], held: false }));
    assert_eq!(drain(&mut routing), [Pointer([60.0, 40.0])]);
    Ok(())
}

#[test]
fn a_gesture_started_in_the_region_stays_captured_until_release() -> TestResult {
    let mut routing = routing(&[MENU])?;
    assert!(routing.route(press(Button::Middle, [50, 40])));
    assert_eq!(routing.cursor(), Some(CursorIntent::ClosedHand));
    assert!(routing.route(Message::Move { point: [15, 25], held: true }), "over an occlusion");
    assert!(routing.route(Message::Move { point: [-40, 900], held: true }), "outside the window");
    assert!(!routing.route(Message::Wheel { point: [-40, 900], delta: 120 }));
    assert!(routing.route(Message::Release {
        button: Button::Middle,
        point: [-40, 900],
        held: false
    }));
    assert_eq!(
        drain(&mut routing),
        [
            Pointer([50.0, 40.0]),
            button(MainCanvasButton::Middle, true),
            Pointer([-40.0, 900.0]),
            Wheel(1.0),
            Pointer([-40.0, 900.0]),
            button(MainCanvasButton::Middle, false),
        ]
    );
    assert_eq!(routing.cursor(), None);
    Ok(())
}

#[test]
fn a_gesture_started_outside_stays_with_webview2_until_every_button_is_released() -> TestResult {
    let mut routing = routing(&[])?;
    assert!(routing.route(press(Button::Primary, [5, 5])));
    assert!(routing.route(Message::Move { point: [50, 40], held: true }));
    assert!(routing.route(Message::Wheel { point: [50, 40], delta: 120 }));
    assert!(routing.route(press(Button::Middle, [50, 40])), "a second button joins the page");
    assert_eq!(drain(&mut routing), []);
    assert_eq!(routing.cursor(), None);
    assert!(routing.route(Message::Release {
        button: Button::Primary,
        point: [50, 40],
        held: true
    }));
    assert!(routing.route(Message::Release {
        button: Button::Middle,
        point: [50, 40],
        held: false
    }));
    assert_eq!(routing.cursor(), Some(CursorIntent::OpenHand));
    assert!(routing.route(Message::Move { point: [51, 40], held: false }));
    assert_eq!(drain(&mut routing), [Pointer([51.0, 40.0])]);
    Ok(())
}

#[test]
fn secondary_and_extra_buttons_always_belong_to_webview2() -> TestResult {
    let mut routing = routing(&[])?;
    for pressed in [Button::Secondary, Button::Extra] {
        assert!(routing.route(press(pressed, [50, 40])));
        assert_eq!(routing.cursor(), None);
        assert!(routing.route(Message::Wheel { point: [50, 40], delta: 120 }));
        assert!(routing.route(Message::Release { button: pressed, point: [50, 40], held: false }));
    }
    assert!(routing.route(press(Button::Primary, [50, 40])));
    assert!(routing.route(press(Button::Secondary, [50, 40])), "context menu during a drag");
    assert!(routing.route(Message::Release {
        button: Button::Secondary,
        point: [50, 40],
        held: true
    }));
    assert_eq!(
        drain(&mut routing),
        [Pointer([50.0, 40.0]), button(MainCanvasButton::Primary, true)]
    );
    assert_eq!(routing.cursor(), Some(CursorIntent::ClosedHand));
    Ok(())
}

#[test]
fn capture_loss_cancels_the_native_gesture_and_its_release_returns_to_webview2() -> TestResult {
    let mut routing = routing(&[])?;
    routing.route(press(Button::Primary, [50, 40]));
    drain(&mut routing);
    routing.cancel();
    routing.cancel();
    assert_eq!(drain(&mut routing), [Cancel], "a single cancellation");
    assert!(routing.route(Message::Release {
        button: Button::Primary,
        point: [50, 40],
        held: false
    }));
    assert_eq!(drain(&mut routing), []);
    assert_eq!(routing.cursor(), Some(CursorIntent::OpenHand));
    Ok(())
}

#[test]
fn withdrawal_cancels_but_a_replaced_region_keeps_the_captured_gesture() -> TestResult {
    let mut routing = routing(&[])?;
    routing.route(press(Button::Primary, [50, 40]));
    drain(&mut routing);
    routing.publish(Some((CLIP, &[PhysicalRect { x: 40, y: 30, width: 20, height: 20 }], false)));
    assert_eq!(routing.cursor(), Some(CursorIntent::ClosedHand));
    assert!(routing.route(Message::Move { point: [52, 41], held: true }));
    assert_eq!(
        drain(&mut routing),
        [Pointer([52.0, 41.0])],
        "still captured over the new occlusion"
    );
    routing.publish(None);
    assert_eq!(drain(&mut routing), [Cancel]);
    assert_eq!(routing.cursor(), None);
    assert!(routing.route(Message::Wheel { point: [52, 41], delta: 120 }), "stale state is web");
    Ok(())
}

#[test]
fn a_new_occlusion_under_the_pointer_restores_the_browser_cursor() -> TestResult {
    let mut routing = routing(&[])?;
    routing.route(Message::Move { point: [15, 25], held: false });
    assert_eq!(routing.cursor(), Some(CursorIntent::OpenHand));
    routing.publish(Some((CLIP, &[MENU], false)));
    assert_eq!(routing.cursor(), None);
    routing.publish(Some((CLIP, &[PhysicalRect { x: 0, y: 0, width: 1, height: 1 }; 65], false)));
    assert_eq!(routing.cursor(), None, "an oversized occlusion list withdraws routing");
    assert!(routing.route(Message::Wheel { point: [80, 60], delta: 120 }));
    Ok(())
}

#[test]
fn leaving_the_window_ends_hover_but_not_a_captured_drag() -> TestResult {
    let mut routing = routing(&[])?;
    routing.route(Message::Move { point: [50, 40], held: false });
    assert!(routing.route(Message::Leave));
    assert_eq!(routing.cursor(), None);
    routing.route(press(Button::Primary, [50, 40]));
    assert!(routing.route(Message::Leave));
    assert_eq!(routing.cursor(), Some(CursorIntent::ClosedHand));
    Ok(())
}

#[test]
fn two_primary_presses_within_the_windows_rule_add_a_double_click() -> TestResult {
    let mut routing = routing(&[])?;
    let release = Message::Release { button: Button::Primary, point: [51, 41], held: false };
    routing.route(press(Button::Primary, [50, 40]));
    routing.route(release);
    drain(&mut routing);
    let second = ClickRule { time: 1_400, ..RULE };
    routing.route(Message::Press { button: Button::Primary, point: [51, 41], rule: second });
    assert_eq!(
        drain(&mut routing),
        [Pointer([51.0, 41.0]), button(MainCanvasButton::Primary, true), DoubleClick([51.0, 41.0])],
        "the press precedes the double-click, as the canvas tool expects"
    );
    routing.route(release);
    let third = ClickRule { time: 1_500, ..RULE };
    routing.route(Message::Press { button: Button::Primary, point: [51, 41], rule: third });
    assert!(!drain(&mut routing).contains(&DoubleClick([51.0, 41.0])), "a pair is consumed");
    routing.route(release);
    let late = ClickRule { time: 2_100, ..RULE };
    routing.route(Message::Press { button: Button::Primary, point: [51, 41], rule: late });
    assert!(!drain(&mut routing).contains(&DoubleClick([51.0, 41.0])), "too slow");
    Ok(())
}

#[test]
fn capture_released_by_the_first_click_keeps_the_double_click_pending() -> TestResult {
    let mut routing = routing(&[])?;
    routing.route(press(Button::Primary, [50, 40]));
    routing.route(Message::Release { button: Button::Primary, point: [50, 40], held: false });
    // Tao releases capture after the button-up, which arrives as a capture change.
    routing.cancel();
    drain(&mut routing);
    let second = ClickRule { time: 1_400, ..RULE };
    routing.route(Message::Press { button: Button::Primary, point: [51, 41], rule: second });
    assert_eq!(
        drain(&mut routing),
        [Pointer([51.0, 41.0]), button(MainCanvasButton::Primary, true), DoubleClick([51.0, 41.0])]
    );
    Ok(())
}

#[test]
fn capture_lost_during_a_drag_or_deactivation_forgets_the_pending_click() -> TestResult {
    let mut routing = routing(&[])?;
    routing.route(press(Button::Primary, [50, 40]));
    routing.cancel();
    assert_eq!(drain(&mut routing).last(), Some(&Cancel));
    let second = ClickRule { time: 1_400, ..RULE };
    routing.route(Message::Press { button: Button::Primary, point: [51, 41], rule: second });
    assert!(!drain(&mut routing).contains(&DoubleClick([51.0, 41.0])), "drag was cancelled");
    routing.route(Message::Release { button: Button::Primary, point: [51, 41], held: false });
    routing.interrupt();
    drain(&mut routing);
    let third = ClickRule { time: 1_500, ..RULE };
    routing.route(Message::Press { button: Button::Primary, point: [51, 41], rule: third });
    assert!(!drain(&mut routing).contains(&DoubleClick([51.0, 41.0])), "window was interrupted");
    Ok(())
}

#[test]
fn moves_coalesce_and_overflow_becomes_one_cancellation() -> TestResult {
    let mut routing = routing(&[])?;
    for x in 20..60 {
        routing.route(Message::Move { point: [x, 40], held: false });
    }
    assert_eq!(routing.pending(), 1);
    assert_eq!(drain(&mut routing), [Pointer([59.0, 40.0])]);
    routing.route(press(Button::Primary, [50, 40]));
    for _ in 0..(QUEUE_CAPACITY - 2) / 2 {
        routing.route(Message::Wheel { point: [50, 40], delta: 120 });
    }
    assert_eq!(routing.pending(), QUEUE_CAPACITY);
    routing.route(Message::Wheel { point: [50, 40], delta: 120 });
    assert_eq!(drain(&mut routing), [Cancel, Wheel(1.0)], "the drag ends; hover zoom remains");
    assert_eq!(routing.cursor(), Some(CursorIntent::OpenHand));
    Ok(())
}

#[test]
fn interruption_cancels_a_native_drag_and_releases_a_web_gesture_hold() -> TestResult {
    let mut routing = routing(&[])?;
    routing.route(press(Button::Primary, [50, 40]));
    drain(&mut routing);
    routing.interrupt();
    assert_eq!(drain(&mut routing), [Cancel]);
    assert!(routing.route(press(Button::Primary, [5, 5])), "a page press outside the region");
    routing.interrupt();
    assert_eq!(drain(&mut routing), [], "no native gesture remained to cancel");
    assert!(!routing.route(Message::Wheel { point: [50, 40], delta: 120 }), "region is native");
    assert_eq!(drain(&mut routing), [Pointer([50.0, 40.0]), Wheel(1.0)]);
    Ok(())
}

#[test]
fn a_fitting_primary_drag_requests_drag_out_once_past_the_threshold() -> TestResult {
    let mut routing = Routing::new()?;
    routing.publish(Some((CLIP, &[], true)));
    assert!(routing.route(Message::Move { point: [50, 40], held: false }));
    assert_eq!(routing.cursor(), Some(CursorIntent::Arrow));
    assert!(routing.route(press(Button::Primary, [50, 40])));
    assert!(routing.route(Message::Move { point: [54, 36], held: true }), "within the threshold");
    assert_eq!(routing.cursor(), Some(CursorIntent::Arrow));
    assert_eq!(
        drain(&mut routing),
        [Pointer([50.0, 40.0]), button(MainCanvasButton::Primary, true), Pointer([54.0, 36.0])]
    );
    assert!(routing.route(Message::Move { point: [55, 40], held: true }));
    assert_eq!(drain(&mut routing), [Pointer([55.0, 40.0]), DragOut]);
    assert_eq!(routing.cursor(), None, "the OS drag owns the remaining press");
    assert!(routing.route(Message::Move { point: [90, 60], held: true }));
    assert!(routing.route(Message::Release {
        button: Button::Primary,
        point: [90, 60],
        held: false
    }));
    assert_eq!(drain(&mut routing), [], "no second request or pan follows");
    Ok(())
}

#[test]
fn a_pannable_or_middle_drag_keeps_panning_without_drag_out() -> TestResult {
    let mut routing = routing(&[])?;
    assert!(routing.route(press(Button::Primary, [50, 40])));
    assert!(routing.route(Message::Move { point: [80, 60], held: true }));
    assert_eq!(routing.cursor(), Some(CursorIntent::ClosedHand));
    assert_eq!(
        drain(&mut routing),
        [Pointer([50.0, 40.0]), button(MainCanvasButton::Primary, true), Pointer([80.0, 60.0])]
    );
    let mut fitting = Routing::new()?;
    fitting.publish(Some((CLIP, &[], true)));
    assert!(fitting.route(press(Button::Middle, [50, 40])));
    assert!(fitting.route(Message::Move { point: [80, 60], held: true }));
    assert_eq!(
        drain(&mut fitting),
        [Pointer([50.0, 40.0]), button(MainCanvasButton::Middle, true), Pointer([80.0, 60.0])]
    );
    Ok(())
}

#[test]
fn losing_drag_out_or_the_region_mid_press_never_requests_it() -> TestResult {
    let mut routing = Routing::new()?;
    routing.publish(Some((CLIP, &[], true)));
    assert!(routing.route(press(Button::Primary, [50, 40])));
    routing.publish(Some((CLIP, &[], false)));
    assert!(routing.route(Message::Move { point: [80, 60], held: true }));
    assert!(!drain(&mut routing).contains(&DragOut));
    assert_eq!(routing.cursor(), Some(CursorIntent::ClosedHand));
    routing.publish(Some((CLIP, &[], true)));
    routing.publish(None);
    assert!(routing.route(Message::Move { point: [90, 60], held: true }));
    assert_eq!(drain(&mut routing), [Cancel]);
    Ok(())
}
