// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Semantic camera destinations preserve physical input semantics and navigation authority.

use super::*;

#[test]
fn freezing_keeps_accepted_pixels_instead_of_sampling_later_motion() -> TestResult {
    let mut nav = navigation()?;
    target(nav.command(NavigationCommand::OneToOne, time(0))?)?;
    nav.advance(time(20))?;
    let accepted = nav.viewport();
    nav.advance(time(40))?;
    assert_ne!(nav.viewport(), accepted);
    nav.freeze_at(accepted, time(60))?;
    assert_eq!(nav.viewport(), accepted);
    assert_eq!(nav.target_viewport(), accepted);
    assert_eq!(nav.zoom_mode(), ZoomMode::Custom);
    assert!(!nav.interacting());
    assert_eq!(nav.next_deadline(time(60)), None);
    nav.advance(time(1000))?;
    assert_eq!(nav.viewport(), accepted);
    Ok(())
}

#[test]
fn freezing_preserves_completed_modes_and_rejects_foreign_geometry_and_time() -> TestResult {
    let mut nav = navigation()?;
    for (command, mode, start) in [
        (NavigationCommand::OneToOne, ZoomMode::OneToOne, 0),
        (NavigationCommand::Fit, ZoomMode::Fit, 100),
    ] {
        target(nav.command(command, time(start))?)?;
        nav.advance(time(start + 80))?;
        let accepted = nav.viewport();
        nav.freeze_at(accepted, time(start + 90))?;
        assert_eq!(nav.zoom_mode(), mode);
        let foreign = Viewport::new(Extent::new(12, 13)?)?;
        assert!(nav.freeze_at(foreign, time(start + 90)).is_err());
        assert!(nav.freeze_at(accepted, time(start + 89)).is_err());
        assert_eq!(nav.viewport(), accepted);
        assert_eq!(nav.zoom_mode(), mode);
    }
    let mut empty = Navigation::new(nav.viewport());
    assert!(empty.freeze_at(nav.viewport(), time(0)).is_err());
    Ok(())
}

fn target(outcome: NavigationCommandOutcome) -> Result<Viewport, CanvasError> {
    match outcome {
        NavigationCommandOutcome::Applied { target } => Ok(target),
        _ => Err(CanvasError::Invalid("Semantic camera command was not admitted")),
    }
}

#[test]
fn semantic_native_scale_centers_the_image_while_physical_input_retains_its_pointer_anchor()
-> TestResult {
    let mut nav = navigation()?;
    nav.input(NavigationInput::Pointer([0.0; 2]), time(0))?;
    nav.input(NavigationInput::OneToOne, time(0))?;
    nav.advance(time(80))?;
    assert_eq!(nav.viewport().zoom(), 1.0);
    assert_eq!(nav.viewport().origin(), [0.0; 2]);
    let centered = target(nav.command(NavigationCommand::OneToOne, time(80))?)?;
    assert_eq!(centered.origin(), [500.0, 250.0]);
    assert_eq!(nav.viewport(), centered);
    assert!(!nav.interacting());
    let fit = target(nav.command(NavigationCommand::Fit, time(90))?)?;
    assert_eq!(fit.zoom(), 0.5);
    assert_eq!(fit.origin(), [0.0; 2]);
    assert_ne!(nav.viewport(), fit);
    nav.advance(time(170))?;
    assert_eq!(nav.viewport(), fit);
    Ok(())
}

#[test]
fn relative_zoom_accumulates_against_pending_intent_without_wheel_stops_or_gesture_lifetime()
-> TestResult {
    let mut nav = navigation()?;
    let first = target(nav.command(NavigationCommand::Zoom(3.0), time(0))?)?;
    assert_eq!(first.zoom(), 1.5);
    let second = target(nav.command(NavigationCommand::Zoom(2.0), time(20))?)?;
    assert_eq!(second.zoom(), 3.0);
    assert_eq!(second.document_at([500.0, 250.0])?, [1000.0, 500.0]);
    nav.advance(time(100))?;
    assert_eq!(nav.viewport(), second);
    assert!(!nav.interacting());
    assert_eq!(nav.next_deadline(time(100)), None);
    assert_eq!(target(nav.command(NavigationCommand::Zoom(100.0), time(100))?)?.zoom(), 10.0);
    nav.advance(time(180))?;
    target(nav.command(NavigationCommand::Zoom(0.01), time(180))?)?;
    let minimum = target(nav.command(NavigationCommand::Zoom(0.01), time(190))?)?;
    assert_eq!(minimum.zoom(), 128.0 / 2000.0);
    Ok(())
}

#[test]
fn fractional_pan_moves_content_in_physical_viewport_units_and_clamps_at_existing_bounds()
-> TestResult {
    let mut nav = navigation()?;
    target(nav.command(NavigationCommand::OneToOne, time(0))?)?;
    nav.advance(time(80))?;
    let moved = target(nav.command(NavigationCommand::Pan([0.1, -0.2]), time(80))?)?;
    assert_eq!(moved.zoom(), 1.0);
    assert_eq!(moved.origin(), [400.0, 350.0]);
    assert_eq!(nav.viewport(), moved);
    let bounded = target(nav.command(NavigationCommand::Pan([10.0, -10.0]), time(80))?)?;
    assert_eq!(bounded.origin(), [-200.0, 600.0]);
    assert_eq!(target(nav.command(NavigationCommand::Pan([0.0; 2]), time(80))?)?, bounded);
    Ok(())
}

#[test]
fn semantic_commands_report_unavailable_owners_and_reject_invalid_parameters_atomically()
-> TestResult {
    let mut empty = Navigation::new(Viewport::new(Extent::new(1000, 500)?)?);
    assert_eq!(
        empty.command(NavigationCommand::Fit, time(0))?,
        NavigationCommandOutcome::NoContent
    );
    let mut nav = navigation()?;
    nav.set_policy(true, false);
    assert_eq!(nav.command(NavigationCommand::Fit, time(0))?, NavigationCommandOutcome::Denied);
    nav.set_policy(false, true);
    assert_eq!(
        nav.command(NavigationCommand::OneToOne, time(0))?,
        NavigationCommandOutcome::Denied
    );
    nav.set_policy(false, false);
    nav.input(NavigationInput::Wheel { steps: 1.0, phase: ScrollPhase::Begin }, time(0))?;
    let before = (nav.viewport(), nav.target_viewport(), nav.next_deadline(time(0)));
    assert_eq!(nav.command(NavigationCommand::Fit, time(10))?, NavigationCommandOutcome::Denied);
    for command in [
        NavigationCommand::Zoom(f64::NAN),
        NavigationCommand::Zoom(0.009),
        NavigationCommand::Zoom(100.01),
        NavigationCommand::Pan([f64::INFINITY, 0.0]),
        NavigationCommand::Pan([0.0, -10.01]),
    ] {
        assert!(nav.command(command, time(10)).is_err());
        assert_eq!((nav.viewport(), nav.target_viewport(), nav.next_deadline(time(0))), before);
    }
    assert!(nav.interacting());
    nav.input(NavigationInput::Cancel, time(10))?;
    target(nav.command(NavigationCommand::OneToOne, time(10))?)?;
    nav.advance(time(90))?;
    nav.input(NavigationInput::Pan(true), time(90))?;
    assert!(nav.captured());
    assert_eq!(nav.command(NavigationCommand::Fit, time(90))?, NavigationCommandOutcome::Denied);
    assert!(nav.captured());
    Ok(())
}

#[test]
fn semantic_cancellation_samples_visible_motion_and_preserves_newer_targets_and_physical_capture()
-> TestResult {
    let mut nav = navigation()?;
    let first = target(nav.command(NavigationCommand::OneToOne, time(0))?)?;
    assert!(nav.cancel_command(first, ZoomMode::OneToOne, time(40))?);
    let stopped = nav.viewport();
    assert!(stopped.zoom() > 0.5 && stopped.zoom() < 1.0);
    assert_eq!(nav.zoom_mode(), ZoomMode::Custom);
    assert_eq!(nav.target_viewport(), stopped);
    assert!(!nav.interacting());
    nav.advance(time(80))?;
    assert_eq!(nav.viewport(), stopped);
    let second = target(nav.command(NavigationCommand::Fit, time(80))?)?;
    assert!(!nav.cancel_command(first, ZoomMode::OneToOne, time(100))?);
    assert_eq!(nav.target_viewport(), second);
    nav.advance(time(160))?;
    target(nav.command(NavigationCommand::OneToOne, time(160))?)?;
    nav.advance(time(240))?;
    let current = nav.viewport();
    nav.input(NavigationInput::Pan(true), time(240))?;
    assert!(nav.captured());
    assert!(!nav.cancel_command(current, ZoomMode::OneToOne, time(240))?);
    assert!(nav.captured());
    Ok(())
}

/// Frame the 2000x1000 fixture content in its 1000x500 viewport from normalized parameters.
fn frame(center: [f64; 2], zoom: [f64; 2]) -> NavigationCommand {
    NavigationCommand::Frame { center, span: [1.0 / (2000.0 * zoom[0]), 1.0 / (1000.0 * zoom[1])] }
}

/// Compare normalized-camera results without depending on reciprocal rounding.
fn close(actual: [f64; 2], expected: [f64; 2]) -> bool {
    (0..2).all(|axis| (actual[axis] - expected[axis]).abs() <= 1e-9 * expected[axis].abs().max(1.0))
}

#[test]
fn framing_places_a_normalized_camera_immediately_without_motion() -> TestResult {
    let mut nav = navigation()?;
    target(nav.command(NavigationCommand::OneToOne, time(0))?)?;
    assert!(nav.interacting());
    let framed = target(nav.command(frame([0.25, 0.75], [2.0, 2.0]), time(10))?)?;
    assert_eq!(nav.viewport(), framed);
    assert!(!nav.interacting());
    assert_eq!(nav.next_deadline(time(10)), None);
    assert_eq!(nav.zoom_mode(), ZoomMode::Custom);
    assert!(close([framed.zoom(); 2], [2.0; 2]));
    assert!(close(framed.document_at([500.0, 250.0])?, [500.0, 750.0]));
    nav.advance(time(1000))?;
    assert_eq!(nav.viewport(), framed);
    let contained = target(nav.command(frame([0.5, 0.5], [3.0, 5.0]), time(1000))?)?;
    assert!(close([contained.zoom(); 2], [3.0; 2]));
    assert!(close(contained.document_at([500.0, 250.0])?, [1000.0, 500.0]));
    Ok(())
}

#[test]
fn framing_snaps_exact_semantic_scales_and_keeps_navigation_bounds() -> TestResult {
    let mut nav = navigation()?;
    target(nav.command(frame([0.1, 0.1], [2.0, 2.0]), time(0))?)?;
    let fit = target(nav.command(frame([0.9, 0.2], [0.5 * (1.0 + 1e-12), 0.5]), time(0))?)?;
    assert_eq!(nav.zoom_mode(), ZoomMode::Fit);
    assert_eq!((fit.zoom(), fit.origin()), (0.5, [0.0; 2]));
    let native = target(nav.command(frame([0.5, 0.5], [1.0 - 1e-12, 1.0]), time(0))?)?;
    assert_eq!(nav.zoom_mode(), ZoomMode::OneToOne);
    assert_eq!((native.zoom(), native.origin()), (1.0, [500.0, 250.0]));
    let closest = target(nav.command(frame([0.5, 0.5], [1e6, 1e6]), time(0))?)?;
    assert_eq!(closest.zoom(), 10.0);
    let farthest = target(nav.command(frame([0.5, 0.5], [1e-6, 1e-6]), time(0))?)?;
    assert_eq!(farthest.zoom(), 0.064);
    let corner = target(nav.command(frame([0.0, 0.0], [2.0, 2.0]), time(0))?)?;
    assert!(close(corner.device_at([0.0, 0.0])?, [200.0, 100.0]));
    let mut small = Navigation::new(Viewport::new(Extent::new(1000, 500)?)?);
    small.set_content(Some(Extent::new(20, 10)?), time(0))?;
    let fitted = small.viewport();
    let framed = target(
        small.command(NavigationCommand::Frame { center: [0.5; 2], span: [1e-9; 2] }, time(0))?,
    )?;
    assert_eq!(framed, fitted);
    assert_eq!(small.zoom_mode(), ZoomMode::Fit);
    Ok(())
}

#[test]
fn framing_rejects_out_of_range_parameters_atomically_and_respects_ownership() -> TestResult {
    let mut nav = navigation()?;
    target(nav.command(NavigationCommand::OneToOne, time(0))?)?;
    let before = (nav.viewport(), nav.target_viewport(), nav.zoom_mode());
    for (center, span) in [
        ([1.000_001, 0.5], [0.001; 2]),
        ([0.5, -0.1], [0.001; 2]),
        ([f64::NAN, 0.5], [0.001; 2]),
        ([0.5; 2], [0.0, 0.001]),
        ([0.5; 2], [0.001, -1.0]),
        ([0.5; 2], [1.000_000_001e9, 0.001]),
        ([0.5; 2], [f64::INFINITY, 0.001]),
    ] {
        assert!(nav.command(NavigationCommand::Frame { center, span }, time(10)).is_err());
        assert_eq!((nav.viewport(), nav.target_viewport(), nav.zoom_mode()), before);
    }
    nav.set_policy(true, false);
    assert_eq!(nav.command(frame([0.5; 2], [2.0; 2]), time(10))?, NavigationCommandOutcome::Denied);
    let mut empty = Navigation::new(Viewport::new(Extent::new(1000, 500)?)?);
    assert_eq!(
        empty.command(frame([0.5; 2], [2.0; 2]), time(0))?,
        NavigationCommandOutcome::NoContent
    );
    Ok(())
}
