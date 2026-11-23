// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The camera follows one responsive curve: every visible change animates, fast input keeps the
//! camera close behind its target, and late frames sample the current point on the curve.
use super::*;

/// One display frame at the maintainer's 144 Hz.
const FRAME_144: Duration = Duration::from_nanos(6_944_444);

/// How far `view` still is from `target`, as a fraction of how far `start` was, in zoom scale.
fn remaining(start: Viewport, view: Viewport, target: Viewport) -> f64 {
    (target.zoom().ln() - view.zoom().ln()) / (target.zoom().ln() - start.zoom().ln())
}

#[test]
fn rapid_wheel_steps_animate_instead_of_jumping() -> TestResult {
    let mut nav = navigation()?;
    nav.set_refresh_rate(Some(144.0))?;
    nav.input(NavigationInput::Pointer([500.0, 250.0]), time(0))?;
    // Zoom in from Fit, so every step changes the scale.
    wheel(&mut nav, 1.0, 0)?;
    for step in 1..10 {
        let at = step * 10;
        wheel(&mut nav, 1.0, at)?;
        let zoom = nav.take_zoom().ok_or("rapid zoom unreported")?;
        assert_eq!(zoom.snapped, None, "step {step} jumped");
        assert!(zoom.duration > Duration::ZERO, "step {step} has no motion");
        let target = nav.target_viewport();
        assert_ne!(nav.viewport(), target, "step {step} landed at once");
        let before = nav.viewport();
        nav.advance(time(at) + FRAME_144 / 2)?;
        let between = nav.viewport();
        assert!(
            (between.zoom() - before.zoom()) * (target.zoom() - between.zoom()) > 0.0,
            "step {step}: before {} between {} target {}",
            before.zoom(),
            between.zoom(),
            target.zoom()
        );
    }
    Ok(())
}

#[test]
fn the_first_display_frame_already_covers_more_than_a_quarter_of_any_zoom() -> TestResult {
    for input in [NavigationInput::OneToOne, NavigationInput::Fit] {
        let mut nav = navigation()?;
        nav.set_refresh_rate(Some(144.0))?;
        nav.input(NavigationInput::Pointer([500.0, 250.0]), time(0))?;
        if input == NavigationInput::Fit {
            nav.input(NavigationInput::OneToOne, time(0))?;
            nav.advance(time(1000))?;
        }
        let start = nav.viewport();
        nav.input(input, time(1000))?;
        let target = nav.target_viewport();
        nav.advance(time(1000) + FRAME_144)?;
        let left = remaining(start, nav.viewport(), target);
        assert!(left < 0.75, "{input:?} covered only {:.0}% in one frame", (1.0 - left) * 100.0);
    }
    Ok(())
}

#[test]
fn fast_wheel_input_keeps_the_camera_within_two_notches_of_its_target() -> TestResult {
    let mut nav = navigation()?;
    nav.set_refresh_rate(Some(144.0))?;
    nav.input(NavigationInput::Pointer([500.0, 250.0]), time(0))?;
    let notch = {
        let mut probe = navigation()?;
        probe.input(NavigationInput::Pointer([500.0, 250.0]), time(0))?;
        wheel(&mut probe, 1.0, 0)?;
        (probe.target_viewport().zoom().ln() - probe.viewport().zoom().ln()).abs()
    };
    for step in 0..12 {
        let at = step * 10;
        wheel(&mut nav, 1.0, at)?;
        nav.advance(time(at) + FRAME_144)?;
        let behind = (nav.target_viewport().zoom().ln() - nav.viewport().zoom().ln()).abs();
        assert!(behind <= 2.0 * notch, "step {step}: {:.2} notches behind", behind / notch);
    }
    Ok(())
}

#[test]
fn a_late_frame_shows_the_current_point_and_every_motion_lands_exactly() -> TestResult {
    let mut steady = navigation()?;
    let mut late = navigation()?;
    for nav in [&mut steady, &mut late] {
        nav.input(NavigationInput::Pointer([500.0, 250.0]), time(0))?;
        nav.input(NavigationInput::OneToOne, time(0))?;
    }
    for ms in 1..40 {
        steady.advance(time(ms))?;
    }
    late.advance(time(39))?;
    assert_eq!(steady.viewport(), late.viewport());
    let end = steady.next_deadline(time(39)).ok_or("motion ended early")?;
    assert!(end > time(39));
    steady.advance(time(1000))?;
    assert_eq!(steady.viewport(), steady.target_viewport());
    assert_eq!(steady.next_deadline(time(1000)), None);
    Ok(())
}
