// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the composed-window routing table without a window, browser or canvas.

use super::*;
use MainCanvasGesture::{Cancel, DoubleClick, DragOut, Wheel};
use sugarsubstitute_canvas::{Pressure, ToolModifiers};

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
    Message::Press { button, sample: sample(point), rule: RULE }
}

fn button(button: MainCanvasButton, pressed: bool) -> MainCanvasGesture {
    MainCanvasGesture::Button { button, pressed, modifiers: ToolModifiers::NONE }
}

fn sample([x, y]: [i32; 2]) -> PointerSample {
    PointerSample::mouse([f64::from(x), f64::from(y)], ToolModifiers::NONE)
}

fn moved(point: [i32; 2], held: bool) -> Message {
    Message::Move { sample: sample(point), held }
}

fn released(button: Button, point: [i32; 2], held: bool) -> Message {
    Message::Release { button, sample: sample(point), held }
}

fn wheel(point: [i32; 2], delta: i16) -> Message {
    Message::Wheel { sample: sample(point), delta }
}

fn pointer(position: [f64; 2]) -> MainCanvasGesture {
    MainCanvasGesture::Pointer(PointerSample::mouse(position, ToolModifiers::NONE))
}

#[test]
fn without_a_region_every_message_belongs_to_webview2_only() -> TestResult {
    let mut routing = Routing::new()?;
    for message in [
        moved([50, 40], false),
        press(Button::Primary, [50, 40]),
        released(Button::Primary, [50, 40], false),
        wheel([50, 40], 120),
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
    assert!(routing.route(moved([50, 40], false)));
    assert!(!routing.route(wheel([51, 41], -60)));
    assert_eq!(drain(&mut routing), [pointer([51.0, 41.0]), Wheel(-0.5)]);
    assert_eq!(routing.cursor(), Some(CursorIntent::OpenHand));
    assert!(routing.route(wheel([5, 5], 120)), "outside the clip");
    assert!(routing.route(Message::Other), "horizontal wheel stays with the page");
    assert_eq!(drain(&mut routing), []);
    Ok(())
}

#[test]
fn occlusions_keep_their_input_and_cursor_with_webview2() -> TestResult {
    let mut routing = routing(&[MENU])?;
    assert!(routing.route(moved([15, 25], false)));
    assert_eq!(routing.cursor(), None);
    assert!(routing.route(press(Button::Primary, [15, 25])));
    assert!(routing.route(wheel([15, 25], 120)));
    assert_eq!(drain(&mut routing), []);
    assert!(routing.route(released(Button::Primary, [15, 25], false)));
    assert!(routing.route(moved([60, 40], false)));
    assert_eq!(drain(&mut routing), [pointer([60.0, 40.0])]);
    Ok(())
}

#[test]
fn a_gesture_started_in_the_region_stays_captured_until_release() -> TestResult {
    let mut routing = routing(&[MENU])?;
    assert!(routing.route(press(Button::Middle, [50, 40])));
    assert_eq!(routing.cursor(), Some(CursorIntent::ClosedHand));
    assert!(routing.route(moved([15, 25], true)), "over an occlusion");
    assert!(routing.route(moved([-40, 900], true)), "outside the window");
    assert!(!routing.route(wheel([-40, 900], 120)));
    assert!(routing.route(released(Button::Middle, [-40, 900], false)));
    assert_eq!(
        drain(&mut routing),
        [
            pointer([50.0, 40.0]),
            button(MainCanvasButton::Middle, true),
            pointer([-40.0, 900.0]),
            Wheel(1.0),
            pointer([-40.0, 900.0]),
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
    assert!(routing.route(moved([50, 40], true)));
    assert!(routing.route(wheel([50, 40], 120)));
    assert!(routing.route(press(Button::Middle, [50, 40])), "a second button joins the page");
    assert_eq!(drain(&mut routing), []);
    assert_eq!(routing.cursor(), None);
    assert!(routing.route(released(Button::Primary, [50, 40], true)));
    assert!(routing.route(released(Button::Middle, [50, 40], false)));
    assert_eq!(routing.cursor(), Some(CursorIntent::OpenHand));
    assert!(routing.route(moved([51, 40], false)));
    assert_eq!(drain(&mut routing), [pointer([51.0, 40.0])]);
    Ok(())
}

#[test]
fn secondary_and_extra_buttons_always_belong_to_webview2() -> TestResult {
    let mut routing = routing(&[])?;
    for pressed in [Button::Secondary, Button::Extra] {
        assert!(routing.route(press(pressed, [50, 40])));
        assert_eq!(routing.cursor(), None);
        assert!(routing.route(wheel([50, 40], 120)));
        assert!(routing.route(released(pressed, [50, 40], false)));
    }
    assert!(routing.route(press(Button::Primary, [50, 40])));
    assert!(routing.route(press(Button::Secondary, [50, 40])), "context menu during a drag");
    assert!(routing.route(released(Button::Secondary, [50, 40], true)));
    assert_eq!(
        drain(&mut routing),
        [pointer([50.0, 40.0]), button(MainCanvasButton::Primary, true)]
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
    assert!(routing.route(released(Button::Primary, [50, 40], false)));
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
    assert!(routing.route(moved([52, 41], true)));
    assert_eq!(
        drain(&mut routing),
        [pointer([52.0, 41.0])],
        "still captured over the new occlusion"
    );
    routing.publish(None);
    assert_eq!(drain(&mut routing), [Cancel]);
    assert_eq!(routing.cursor(), None);
    assert!(routing.route(wheel([52, 41], 120)), "stale state is web");
    Ok(())
}

#[test]
fn a_new_occlusion_under_the_pointer_restores_the_browser_cursor() -> TestResult {
    let mut routing = routing(&[])?;
    routing.route(moved([15, 25], false));
    assert_eq!(routing.cursor(), Some(CursorIntent::OpenHand));
    routing.publish(Some((CLIP, &[MENU], false)));
    assert_eq!(routing.cursor(), None);
    routing.publish(Some((CLIP, &[PhysicalRect { x: 0, y: 0, width: 1, height: 1 }; 65], false)));
    assert_eq!(routing.cursor(), None, "an oversized occlusion list withdraws routing");
    assert!(routing.route(wheel([80, 60], 120)));
    Ok(())
}

#[test]
fn leaving_the_window_ends_hover_but_not_a_captured_drag() -> TestResult {
    let mut routing = routing(&[])?;
    routing.route(moved([50, 40], false));
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
    let release = released(Button::Primary, [51, 41], false);
    routing.route(press(Button::Primary, [50, 40]));
    routing.route(release);
    drain(&mut routing);
    let second = ClickRule { time: 1_400, ..RULE };
    routing.route(Message::Press {
        button: Button::Primary,
        sample: sample([51, 41]),
        rule: second,
    });
    assert_eq!(
        drain(&mut routing),
        [pointer([51.0, 41.0]), button(MainCanvasButton::Primary, true), DoubleClick([51.0, 41.0])],
        "the press precedes the double-click, as the canvas tool expects"
    );
    routing.route(release);
    let third = ClickRule { time: 1_500, ..RULE };
    routing.route(Message::Press {
        button: Button::Primary,
        sample: sample([51, 41]),
        rule: third,
    });
    assert!(!drain(&mut routing).contains(&DoubleClick([51.0, 41.0])), "a pair is consumed");
    routing.route(release);
    let late = ClickRule { time: 2_100, ..RULE };
    routing.route(Message::Press { button: Button::Primary, sample: sample([51, 41]), rule: late });
    assert!(!drain(&mut routing).contains(&DoubleClick([51.0, 41.0])), "too slow");
    Ok(())
}

#[test]
fn capture_released_by_the_first_click_keeps_the_double_click_pending() -> TestResult {
    let mut routing = routing(&[])?;
    routing.route(press(Button::Primary, [50, 40]));
    routing.route(released(Button::Primary, [50, 40], false));
    // Tao releases capture after the button-up, which arrives as a capture change.
    routing.cancel();
    drain(&mut routing);
    let second = ClickRule { time: 1_400, ..RULE };
    routing.route(Message::Press {
        button: Button::Primary,
        sample: sample([51, 41]),
        rule: second,
    });
    assert_eq!(
        drain(&mut routing),
        [pointer([51.0, 41.0]), button(MainCanvasButton::Primary, true), DoubleClick([51.0, 41.0])]
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
    routing.route(Message::Press {
        button: Button::Primary,
        sample: sample([51, 41]),
        rule: second,
    });
    assert!(!drain(&mut routing).contains(&DoubleClick([51.0, 41.0])), "drag was cancelled");
    routing.route(released(Button::Primary, [51, 41], false));
    routing.interrupt();
    drain(&mut routing);
    let third = ClickRule { time: 1_500, ..RULE };
    routing.route(Message::Press {
        button: Button::Primary,
        sample: sample([51, 41]),
        rule: third,
    });
    assert!(!drain(&mut routing).contains(&DoubleClick([51.0, 41.0])), "window was interrupted");
    Ok(())
}

#[test]
fn moves_coalesce_and_overflow_becomes_one_cancellation() -> TestResult {
    let mut routing = routing(&[])?;
    for x in 20..60 {
        routing.route(moved([x, 40], false));
    }
    assert_eq!(routing.pending(), 1);
    assert_eq!(drain(&mut routing), [pointer([59.0, 40.0])]);
    routing.route(press(Button::Primary, [50, 40]));
    for _ in 0..(QUEUE_CAPACITY - 2) / 2 {
        routing.route(wheel([50, 40], 120));
    }
    assert_eq!(routing.pending(), QUEUE_CAPACITY);
    routing.route(wheel([50, 40], 120));
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
    assert!(!routing.route(wheel([50, 40], 120)), "region is native");
    assert_eq!(drain(&mut routing), [pointer([50.0, 40.0]), Wheel(1.0)]);
    Ok(())
}

#[test]
fn a_fitting_primary_drag_requests_drag_out_once_past_the_threshold() -> TestResult {
    let mut routing = Routing::new()?;
    routing.publish(Some((CLIP, &[], true)));
    assert!(routing.route(moved([50, 40], false)));
    assert_eq!(routing.cursor(), Some(CursorIntent::Arrow));
    assert!(routing.route(press(Button::Primary, [50, 40])));
    assert!(routing.route(moved([54, 36], true)), "within the threshold");
    assert_eq!(routing.cursor(), Some(CursorIntent::Arrow));
    assert_eq!(
        drain(&mut routing),
        [pointer([50.0, 40.0]), button(MainCanvasButton::Primary, true), pointer([54.0, 36.0])]
    );
    assert!(routing.route(moved([55, 40], true)));
    assert_eq!(drain(&mut routing), [pointer([55.0, 40.0]), DragOut]);
    assert_eq!(routing.cursor(), None, "the OS drag owns the remaining press");
    assert!(routing.route(moved([90, 60], true)));
    assert!(routing.route(released(Button::Primary, [90, 60], false)));
    assert_eq!(drain(&mut routing), [], "no second request or pan follows");
    Ok(())
}

#[test]
fn a_pannable_or_middle_drag_keeps_panning_without_drag_out() -> TestResult {
    let mut routing = routing(&[])?;
    assert!(routing.route(press(Button::Primary, [50, 40])));
    assert!(routing.route(moved([80, 60], true)));
    assert_eq!(routing.cursor(), Some(CursorIntent::ClosedHand));
    assert_eq!(
        drain(&mut routing),
        [pointer([50.0, 40.0]), button(MainCanvasButton::Primary, true), pointer([80.0, 60.0])]
    );
    let mut fitting = Routing::new()?;
    fitting.publish(Some((CLIP, &[], true)));
    assert!(fitting.route(press(Button::Middle, [50, 40])));
    assert!(fitting.route(moved([80, 60], true)));
    assert_eq!(
        drain(&mut fitting),
        [pointer([50.0, 40.0]), button(MainCanvasButton::Middle, true), pointer([80.0, 60.0])]
    );
    Ok(())
}

#[test]
fn losing_drag_out_or_the_region_mid_press_never_requests_it() -> TestResult {
    let mut routing = Routing::new()?;
    routing.publish(Some((CLIP, &[], true)));
    assert!(routing.route(press(Button::Primary, [50, 40])));
    routing.publish(Some((CLIP, &[], false)));
    assert!(routing.route(moved([80, 60], true)));
    assert!(!drain(&mut routing).contains(&DragOut));
    assert_eq!(routing.cursor(), Some(CursorIntent::ClosedHand));
    routing.publish(Some((CLIP, &[], true)));
    routing.publish(None);
    assert!(routing.route(moved([90, 60], true)));
    assert_eq!(drain(&mut routing), [Cancel]);
    Ok(())
}

fn pen([x, y]: [i32; 2], pressure: u32, eraser: bool) -> PointerSample {
    PointerSample {
        position: [f64::from(x), f64::from(y)],
        kind: PointerKind::Pen,
        pressure: Pressure::from_range(pressure, 1024),
        eraser,
        modifiers: ToolModifiers { shift: true, ctrl: false, alt: true },
    }
}

#[test]
fn pen_hover_and_contact_inside_the_region_reach_only_the_canvas_with_their_samples() -> TestResult
{
    let mut routing = routing(&[MENU])?;
    let hover = pen([50, 40], 0, false);
    assert!(!routing.route_pen(Message::Move { sample: hover, held: false }));
    let contact = pen([50, 40], 512, true);
    assert!(!routing.route_pen(Message::Press {
        button: Button::Primary,
        sample: contact,
        rule: RULE
    }));
    let stroke = pen([-40, 900], 1024, true);
    assert!(!routing.route_pen(Message::Move { sample: stroke, held: true }), "captured outside");
    let lifted = pen([-40, 900], 0, true);
    assert!(!routing.route_pen(Message::Release {
        button: Button::Primary,
        sample: lifted,
        held: false
    }));
    let modifiers = ToolModifiers { shift: true, ctrl: false, alt: true };
    assert_eq!(
        drain(&mut routing),
        [
            MainCanvasGesture::Pointer(contact),
            MainCanvasGesture::Button {
                button: MainCanvasButton::Primary,
                pressed: true,
                modifiers
            },
            MainCanvasGesture::Pointer(lifted),
            MainCanvasGesture::Button {
                button: MainCanvasButton::Primary,
                pressed: false,
                modifiers
            },
        ],
        "consecutive samples coalesce to the latest before each button transition"
    );
    assert!(routing.route_pen(Message::Move { sample: pen([15, 25], 0, false), held: false }));
    assert!(routing.route_pen(Message::Leave));
    assert_eq!(drain(&mut routing), [], "occluded hover and range exit belong to the page");
    Ok(())
}

#[test]
fn pen_contacts_outside_the_region_and_barrel_presses_stay_with_webview2() -> TestResult {
    let mut routing = routing(&[])?;
    assert!(routing.route_pen(Message::Press {
        button: Button::Primary,
        sample: pen([5, 5], 300, false),
        rule: RULE
    }));
    assert!(routing.route_pen(Message::Move { sample: pen([50, 40], 300, false), held: true }));
    assert!(routing.route_pen(Message::Release {
        button: Button::Primary,
        sample: pen([50, 40], 0, false),
        held: false
    }));
    assert!(routing.route_pen(Message::Press {
        button: Button::Secondary,
        sample: pen([50, 40], 300, false),
        rule: RULE
    }));
    assert!(routing.route_pen(Message::Release {
        button: Button::Secondary,
        sample: pen([50, 40], 0, false),
        held: false
    }));
    assert_eq!(drain(&mut routing), []);
    Ok(())
}

#[test]
fn a_pen_contact_pans_instead_of_requesting_drag_out() -> TestResult {
    let mut routing = Routing::new()?;
    routing.publish(Some((CLIP, &[], true)));
    let contact = pen([50, 40], 700, false);
    assert!(!routing.route_pen(Message::Press {
        button: Button::Primary,
        sample: contact,
        rule: RULE
    }));
    let moved = pen([90, 60], 700, false);
    assert!(!routing.route_pen(Message::Move { sample: moved, held: true }));
    assert!(!drain(&mut routing).contains(&DragOut));
    assert!(!routing.route_pen(Message::Release {
        button: Button::Primary,
        sample: moved,
        held: false
    }));
    Ok(())
}

#[test]
fn mouse_button_gestures_carry_the_modifiers_sampled_with_their_messages() -> TestResult {
    let mut routing = routing(&[])?;
    let modifiers = ToolModifiers { shift: false, ctrl: true, alt: false };
    let held = PointerSample::mouse([50.0, 40.0], modifiers);
    routing.route(Message::Press { button: Button::Middle, sample: held, rule: RULE });
    let free = PointerSample::mouse([50.0, 40.0], ToolModifiers::NONE);
    routing.route(Message::Release { button: Button::Middle, sample: free, held: false });
    assert_eq!(
        drain(&mut routing),
        [
            MainCanvasGesture::Pointer(held),
            MainCanvasGesture::Button {
                button: MainCanvasButton::Middle,
                pressed: true,
                modifiers
            },
            MainCanvasGesture::Pointer(free),
            button(MainCanvasButton::Middle, false),
        ]
    );
    Ok(())
}
