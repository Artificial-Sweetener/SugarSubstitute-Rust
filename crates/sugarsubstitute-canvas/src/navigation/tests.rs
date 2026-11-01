// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Characterize navigation with explicit timestamps, geometry and semantic destinations.
use super::*;
mod commands;

#[test]
fn cancelling_an_incomplete_stop_keeps_the_visible_camera_in_custom_mode() -> TestResult {
    let mut nav = navigation()?;
    actual(&mut nav)?;
    nav.input(NavigationInput::Fit, time(100))?;
    nav.advance(time(140))?;
    let visible = nav.viewport();
    nav.input(NavigationInput::Cancel, time(140))?;
    assert_eq!(nav.zoom_mode(), ZoomMode::Custom);
    assert_eq!(nav.viewport(), visible);
    assert_eq!(nav.next_deadline(time(140)), None);
    nav.resize(Extent::new(1100, 600)?, time(150))?;
    assert_eq!(nav.viewport().zoom(), visible.zoom());
    nav.input(NavigationInput::DoubleClick([550.0, 300.0]), time(160))?;
    nav.resize(Extent::new(1200, 600)?, time(180))?;
    assert_eq!(nav.zoom_mode(), ZoomMode::Fit);
    assert!(nav.next_deadline(time(180)).is_some());
    nav.advance(time(240))?;
    assert_eq!(nav.viewport().zoom(), 0.6);
    Ok(())
}

#[test]
fn cadence_retargets_without_a_queue_and_uses_exact_elapsed_endpoints() -> TestResult {
    for (cadence, duration) in [(25, 0), (26, 20), (40, 20), (100, 50), (160, 80)] {
        let mut nav = navigation()?;
        wheel(&mut nav, 1.0, 0)?;
        wheel(&mut nav, 1.0, cadence)?;
        let target = nav.target_viewport();
        assert_eq!(target.zoom(), 0.78125);
        if duration > 0 {
            assert_ne!(nav.viewport(), target);
            nav.advance(time(cadence + duration - 1))?;
            assert_ne!(nav.viewport(), target);
        }
        nav.advance(time(cadence + duration))?;
        assert_eq!(nav.viewport(), target);
    }
    Ok(())
}

#[test]
fn fit_motion_interpolates_physical_pan_and_late_ticks_skip_to_endpoint() -> TestResult {
    let mut nav = navigation()?;
    actual(&mut nav)?;
    nav.input(NavigationInput::Pan(true), time(90))?;
    nav.input(NavigationInput::Pointer([600.0, 290.0]), time(91))?;
    nav.input(NavigationInput::Pan(false), time(92))?;
    let start = nav.viewport();
    nav.input(NavigationInput::Fit, time(100))?;
    nav.advance(time(140))?;
    let middle = nav.viewport();
    let pan = |view: Viewport| {
        [
            (1000.0 - view.origin()[0]) * view.zoom() - 500.0,
            (500.0 - view.origin()[1]) * view.zoom() - 250.0,
        ]
    };
    for (current, original) in pan(middle).into_iter().zip(pan(start)) {
        assert!((current - original * 0.25).abs() < 1e-10);
    }
    assert!(nav.next_deadline(time(140)).is_some_and(|deadline| deadline > time(140)));
    nav.advance(time(1000))?;
    assert_eq!(nav.viewport().zoom(), 0.5);
    assert_eq!(nav.viewport().origin(), [0.0; 2]);
    assert_eq!(nav.next_deadline(time(1000)), None);
    Ok(())
}

#[test]
fn final_phased_wheel_delta_is_not_lost_when_gesture_ends() -> TestResult {
    let mut nav = navigation()?;
    nav.input(NavigationInput::Wheel { steps: 1.0, phase: ScrollPhase::Begin }, time(0))?;
    nav.input(NavigationInput::Wheel { steps: 0.5, phase: ScrollPhase::End }, time(10))?;
    assert_eq!(nav.viewport().zoom(), 0.625 * 1.25_f64.sqrt());
    assert!(!nav.interacting());
    assert_eq!(nav.next_deadline(time(10)), None);
    Ok(())
}
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn irregular_slow_frames_sample_elapsed_motion_and_never_queue_missed_refresh_ticks() -> TestResult
{
    for hz in [30.0, 60.0, 120.0, 144.0, 240.0] {
        let mut slow = navigation()?;
        slow.set_refresh_rate(Some(hz))?;
        let pointer = [313.25, 219.125];
        slow.input(NavigationInput::Pointer(pointer), time(0))?;
        let anchor = slow.viewport().document_at(pointer)?;
        wheel(&mut slow, 1.0, 0)?;
        let endpoint = slow.target_viewport();
        for ms in [13, 42, 79] {
            slow.advance(time(ms))?;
            let t = ms as f64 / 80.0;
            let expected = 0.5 + 0.125 * (1.0 - (1.0 - t).powi(2));
            assert!((slow.viewport().zoom() - expected).abs() < 1e-12);
            assert_eq!(slow.viewport().document_at(pointer)?, anchor);
            let next = slow.next_deadline(time(ms)).ok_or("Missing next animation deadline")?;
            assert!(next > time(ms));
            assert!(next <= time(80));
        }
        slow.advance(time(250))?;
        assert_eq!(slow.viewport(), endpoint);
        assert_eq!(slow.next_deadline(time(250)), None);
        assert!(!slow.interacting());
        wheel(&mut slow, 1.0, 250)?;
        assert_eq!(slow.target_viewport().zoom(), 0.78125);
    }
    Ok(())
}

fn time(ms: u64) -> Duration {
    Duration::from_millis(ms)
}
fn navigation() -> Result<Navigation, CanvasError> {
    let mut nav = Navigation::new(Viewport::new(Extent::new(1000, 500)?)?);
    nav.set_content(Some(Extent::new(2000, 1000)?), time(0))?;
    Ok(nav)
}
fn wheel(nav: &mut Navigation, steps: f64, ms: u64) -> Result<(), CanvasError> {
    nav.input(NavigationInput::Wheel { steps, phase: ScrollPhase::Unphased }, time(ms))
}
fn actual(nav: &mut Navigation) -> Result<(), CanvasError> {
    nav.input(NavigationInput::Pointer([500.0, 250.0]), time(0))?;
    nav.input(NavigationInput::OneToOne, time(0))?;
    nav.advance(time(80))
}

#[test]
fn cursor_and_capture_follow_pan_availability_and_cancel_without_sticky_state() -> TestResult {
    let mut nav = navigation()?;
    assert_eq!(nav.cursor(), CursorIntent::Arrow);
    nav.input(NavigationInput::Pan(true), time(0))?;
    assert!(!nav.captured());
    actual(&mut nav)?;
    assert_eq!(nav.cursor(), CursorIntent::OpenHand);
    nav.input(NavigationInput::Pan(true), time(90))?;
    assert_eq!(nav.cursor(), CursorIntent::ClosedHand);
    nav.input(NavigationInput::Pointer([513.0, 243.0]), time(91))?;
    assert_eq!(nav.viewport().origin(), [487.0, 257.0]);
    nav.input(NavigationInput::Pan(false), time(92))?;
    assert_eq!(nav.cursor(), CursorIntent::OpenHand);
    nav.input(NavigationInput::Pan(true), time(93))?;
    nav.input(NavigationInput::Cancel, time(94))?;
    assert!(!nav.interacting());
    assert_eq!(nav.cursor(), CursorIntent::OpenHand);
    nav.set_policy(true, false);
    assert_eq!(nav.cursor(), CursorIntent::Arrow);
    nav.input(NavigationInput::Pan(true), time(95))?;
    assert!(!nav.captured());
    nav.set_policy(false, true);
    assert_eq!(nav.cursor(), CursorIntent::Arrow);
    Ok(())
}

#[test]
fn drag_out_applies_only_to_unlocked_content_that_fits_the_surface() -> TestResult {
    let mut empty = Navigation::new(Viewport::new(Extent::new(1000, 500)?)?);
    assert!(!empty.allows_drag_out(), "no content");
    empty.set_content(Some(Extent::new(400, 200)?), time(0))?;
    assert!(empty.allows_drag_out(), "a small image fits even at 1:1");
    let mut nav = navigation()?;
    assert!(nav.allows_drag_out(), "Fit keeps the content within the surface");
    actual(&mut nav)?;
    assert!(!nav.allows_drag_out(), "1:1 of a larger image pans instead");
    nav.input(NavigationInput::Fit, time(100))?;
    nav.advance(time(400))?;
    assert!(nav.allows_drag_out());
    nav.set_policy(true, false);
    assert!(!nav.allows_drag_out(), "locked previews never drag out");
    Ok(())
}

#[test]
fn wheel_magnitude_accumulates_pending_intent_and_leaves_exact_native_snap() -> TestResult {
    let mut nav = navigation()?;
    wheel(&mut nav, 3.0, 0)?;
    assert_eq!(nav.target_viewport().zoom(), 0.9765625);
    wheel(&mut nav, 1.0, 10)?;
    assert_eq!(nav.viewport().zoom(), 1.0);
    assert_eq!(nav.zoom_mode(), ZoomMode::OneToOne);
    wheel(&mut nav, 0.5, 20)?;
    assert_eq!(nav.zoom_mode(), ZoomMode::Custom);
    assert!((nav.viewport().zoom() - 1.25_f64.sqrt()).abs() < 1e-14);
    Ok(())
}

#[test]
fn fit_crossings_snap_in_both_directions_and_recenter() -> TestResult {
    let mut nav = navigation()?;
    wheel(&mut nav, -1.0, 0)?;
    nav.advance(time(80))?;
    assert_eq!(nav.viewport().zoom(), 0.4);
    assert_eq!(nav.zoom_mode(), ZoomMode::Custom);
    wheel(&mut nav, 1.0, 200)?;
    assert_eq!(nav.target_viewport().zoom(), 0.5);
    assert_eq!(nav.zoom_mode(), ZoomMode::Fit);
    nav.advance(time(280))?;
    assert_eq!(nav.viewport().origin(), [0.0, 0.0]);
    wheel(&mut nav, 1.0, 400)?;
    nav.advance(time(480))?;
    assert_eq!(nav.viewport().zoom(), 0.625);
    wheel(&mut nav, -1.0, 600)?;
    assert_eq!(nav.zoom_mode(), ZoomMode::Fit);
    assert_eq!(nav.target_viewport().zoom(), 0.5);
    Ok(())
}

#[test]
fn native_stop_wins_when_one_event_crosses_both_fit_and_native() -> TestResult {
    let mut nav = Navigation::new(Viewport::new(Extent::new(1050, 525)?)?);
    nav.set_content(Some(Extent::new(1000, 500)?), time(0))?;
    nav.input(NavigationInput::OneToOne, time(0))?;
    nav.advance(time(80))?;
    wheel(&mut nav, -1.0, 100)?;
    nav.advance(time(180))?;
    assert_eq!(nav.viewport().zoom(), 0.8);
    wheel(&mut nav, 2.0, 220)?;
    assert_eq!(nav.target_viewport().zoom(), 1.0);
    assert_eq!(nav.zoom_mode(), ZoomMode::OneToOne);
    Ok(())
}

#[test]
fn double_click_uses_semantic_mode_and_preserves_anchor_through_easing() -> TestResult {
    let mut nav = navigation()?;
    let anchor = [400.0, 250.0];
    let point = nav.viewport().document_at(anchor)?;
    nav.input(NavigationInput::DoubleClick(anchor), time(0))?;
    assert_eq!(nav.zoom_mode(), ZoomMode::OneToOne);
    nav.advance(time(40))?;
    assert_eq!(nav.viewport().zoom(), 0.875);
    let current = nav.viewport().document_at(anchor)?;
    assert!((current[0] - point[0]).abs() < 1e-10);
    assert!((current[1] - point[1]).abs() < 1e-10);
    nav.advance(time(800))?;
    assert_eq!(nav.viewport().zoom(), 1.0);
    nav.input(NavigationInput::DoubleClick(anchor), time(900))?;
    assert_eq!(nav.zoom_mode(), ZoomMode::Fit);
    nav.advance(time(980))?;
    assert_eq!(nav.viewport().zoom(), 0.5);
    assert_eq!(nav.viewport().origin(), [0.0; 2]);
    Ok(())
}

#[test]
fn fit_resize_and_numerically_equal_native_scale_keep_distinct_modes() -> TestResult {
    let mut nav = navigation()?;
    nav.resize(Extent::new(2000, 1000)?, time(0))?;
    assert_eq!(nav.viewport().zoom(), 1.0);
    assert_eq!(nav.zoom_mode(), ZoomMode::Fit);
    nav.input(NavigationInput::DoubleClick([1000.0, 500.0]), time(0))?;
    assert_eq!(nav.viewport().zoom(), 1.0);
    assert_eq!(nav.zoom_mode(), ZoomMode::OneToOne);
    nav.resize(Extent::new(1000, 500)?, time(0))?;
    assert_eq!(nav.viewport().zoom(), 1.0);
    assert_eq!(nav.viewport().document_at([500.0, 250.0])?, [1000.0, 500.0]);
    Ok(())
}

#[test]
fn explicit_wheel_lifetime_outlasts_idle_and_end_does_not_discard_motion() -> TestResult {
    let mut nav = navigation()?;
    nav.input(NavigationInput::Wheel { steps: 1.0, phase: ScrollPhase::Begin }, time(0))?;
    nav.advance(time(300))?;
    assert!(nav.interacting());
    assert_eq!(nav.next_deadline(time(300)), None);
    nav.input(NavigationInput::Wheel { steps: 1.0, phase: ScrollPhase::Update }, time(400))?;
    nav.input(NavigationInput::Wheel { steps: 0.0, phase: ScrollPhase::End }, time(410))?;
    assert!(nav.interacting());
    nav.advance(time(480))?;
    assert!(!nav.interacting());
    wheel(&mut nav, -1.0, 500)?;
    nav.advance(time(619))?;
    assert!(nav.interacting());
    nav.advance(time(620))?;
    assert!(!nav.interacting());
    Ok(())
}

#[test]
fn fractional_refresh_periods_retime_without_restarting_animation() -> TestResult {
    let mut nav = navigation()?;
    nav.set_refresh_rate(Some(144.0))?;
    nav.input(NavigationInput::OneToOne, time(0))?;
    assert_eq!(nav.next_deadline(time(0)), Some(Duration::from_nanos(6_944_444)));
    nav.advance(time(40))?;
    assert_eq!(nav.viewport().zoom(), 0.875);
    nav.set_refresh_rate(Some(120.0))?;
    assert_eq!(nav.next_deadline(time(40)), Some(Duration::from_nanos(41_666_665)));
    nav.advance(time(80))?;
    assert_eq!(nav.viewport().zoom(), 1.0);
    assert_eq!(nav.next_deadline(time(80)), None);
    Ok(())
}

#[test]
fn invalid_observations_and_backward_time_do_not_mutate_camera_or_capture() -> TestResult {
    let mut nav = navigation()?;
    actual(&mut nav)?;
    nav.input(NavigationInput::Pan(true), time(90))?;
    let previous = nav.viewport();
    for input in [
        NavigationInput::Pointer([f64::NAN, 0.0]),
        NavigationInput::Wheel { steps: f64::INFINITY, phase: ScrollPhase::Begin },
    ] {
        assert!(nav.input(input, time(100)).is_err());
        assert_eq!(nav.viewport(), previous);
        assert!(nav.captured());
    }
    assert!(nav.advance(time(89)).is_err());
    assert_eq!(nav.viewport(), previous);
    assert!(nav.set_refresh_rate(Some(f64::NAN)).is_err());
    Ok(())
}

#[test]
fn integer_drag_preserves_exact_blit_phase_and_small_native_images_stay_centered() -> TestResult {
    let mut nav = navigation()?;
    actual(&mut nav)?;
    let previous = nav.viewport();
    nav.input(NavigationInput::Pan(true), time(90))?;
    nav.input(NavigationInput::Pointer([513.0, 243.0]), time(91))?;
    assert!(nav.viewport().repair_from(previous)?.blit().is_some());
    nav.set_content(Some(Extent::new(100, 100)?), time(100))?;
    nav.input(NavigationInput::OneToOne, time(100))?;
    nav.advance(time(180))?;
    assert!(!nav.can_pan());
    assert_eq!(nav.cursor(), CursorIntent::Arrow);
    assert_eq!(nav.viewport().origin(), [-450.0, -200.0]);
    let centered = nav.viewport();
    nav.input(NavigationInput::Pan(true), time(190))?;
    assert!(!nav.captured());
    nav.input(NavigationInput::Pointer([100_000.0; 2]), time(191))?;
    assert_eq!(nav.viewport(), centered);
    Ok(())
}

#[test]
fn one_axis_overflow_admits_hand_capture_and_resize_to_fit_releases_it() -> TestResult {
    let mut nav = Navigation::new(Viewport::new(Extent::new(1000, 500)?)?);
    nav.set_content(Some(Extent::new(1200, 100)?), time(0))?;
    nav.input(NavigationInput::OneToOne, time(0))?;
    nav.advance(time(80))?;
    assert!(nav.can_pan());
    assert_eq!(nav.cursor(), CursorIntent::OpenHand);

    nav.input(NavigationInput::Pan(true), time(90))?;
    assert!(nav.captured());
    assert_eq!(nav.cursor(), CursorIntent::ClosedHand);
    nav.resize(Extent::new(1200, 500)?, time(91))?;
    assert!(!nav.captured());
    assert!(!nav.can_pan());
    assert_eq!(nav.cursor(), CursorIntent::Arrow);
    assert_eq!(nav.viewport().origin(), [0.0, -200.0]);
    Ok(())
}

#[test]
fn geometry_and_policy_changes_do_not_discard_zoom_transitions() -> TestResult {
    let mut nav = navigation()?;
    actual(&mut nav)?;
    nav.input(NavigationInput::Pan(true), time(90))?;
    nav.input(NavigationInput::Wheel { steps: -4.0, phase: ScrollPhase::Begin }, time(100))?;
    assert!(!nav.captured());
    assert_eq!(nav.zoom_mode(), ZoomMode::Fit);
    assert!(nav.next_deadline(time(100)).is_some());
    nav.resize(Extent::new(1200, 600)?, time(120))?;
    assert!(nav.next_deadline(time(120)).is_some());
    nav.set_policy(true, false);
    assert!(nav.next_deadline(time(120)).is_some());
    assert_eq!(nav.cursor(), CursorIntent::Arrow);
    nav.advance(time(180))?;
    assert_eq!(nav.zoom_mode(), ZoomMode::Fit);
    assert_eq!(nav.viewport().zoom(), 0.6);
    Ok(())
}

#[test]
fn returning_to_fit_preserves_the_original_large_image_sampling_phase() -> TestResult {
    let mut nav = Navigation::new(Viewport::new(Extent::new(3840, 2160)?)?);
    nav.set_content(Some(Extent::new(16384, 16384)?), time(0))?;
    let fitted = nav.viewport();
    nav.input(NavigationInput::Fit, time(1))?;
    assert_eq!(nav.viewport(), fitted);
    nav.input(NavigationInput::OneToOne, time(10))?;
    nav.advance(time(90))?;
    nav.input(NavigationInput::Fit, time(100))?;
    nav.advance(time(180))?;
    assert_eq!(nav.viewport(), fitted);
    nav.input(NavigationInput::Wheel { steps: 1.0, phase: ScrollPhase::Begin }, time(200))?;
    nav.advance(time(280))?;
    nav.input(NavigationInput::Wheel { steps: -1.0, phase: ScrollPhase::End }, time(300))?;
    nav.advance(time(380))?;
    assert_eq!(nav.zoom_mode(), ZoomMode::Fit);
    assert_eq!(nav.viewport(), fitted);
    assert_eq!(nav.viewport().repair_from(fitted)?.dirty().pixels(), 0);
    Ok(())
}
