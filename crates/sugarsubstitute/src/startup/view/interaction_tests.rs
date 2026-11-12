// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise mascot input and disclosure with deterministic presentation timestamps.
use super::*;
use std::error::Error;
use sugarsubstitute_execution::qualification_time::anchor;

fn view(reduced: bool) -> Result<StartupView, Box<dyn Error>> {
    Ok(StartupView::new(
        Presentation::new("en", RequestedTheme::Dark, reduced)?,
        sugarsubstitute_ui::accent::AccentPalette::PRODUCT,
    ))
}

#[test]
fn mascot_press_flips_once_and_keeps_native_drag_eligibility() -> Result<(), Box<dyn Error>> {
    let mut view = view(false)?;
    let mascot = Pointer { x: 270.0, y: 200.0 };
    let original = view.animation.frame(Duration::from_millis(100));
    assert_eq!(view.hit_test(mascot), HitTarget::Caption);
    assert!(view.pointer(PointerAction::Press, mascot));
    assert!(!view.pointer(PointerAction::Press, mascot));
    view.pointer(PointerAction::Leave, mascot);
    assert_ne!(view.animation.frame(Duration::from_millis(100)), original);
    assert_eq!(view.hit_test(mascot), HitTarget::Caption);
    Ok(())
}

#[test]
fn console_controls_and_background_do_not_trigger_mascot() -> Result<(), Box<dyn Error>> {
    for point in
        [Pointer { x: 5.0, y: 200.0 }, Pointer { x: 480.0, y: 12.0 }, Pointer { x: 530.0, y: 12.0 }]
    {
        let mut view = view(false)?;
        let original = view.animation.frame(Duration::from_millis(100));
        view.pointer(PointerAction::Press, point);
        assert_eq!(view.animation.frame(Duration::from_millis(100)), original);
    }
    let mut view = view(false)?;
    view.activate(1);
    view.sample(StartupPhase::PreparingConnection, Duration::from_millis(220), anchor());
    view.publish_controls(view.appearance);
    let original = view.animation.frame(Duration::from_millis(320));
    assert!(!view.pointer(PointerAction::Press, Pointer { x: 270.0, y: 380.0 }));
    assert_eq!(view.animation.frame(Duration::from_millis(320)), original);
    Ok(())
}

#[test]
fn disclosure_reverses_continuously_and_native_geometry_waits_for_accepted_frame()
-> Result<(), Box<dyn Error>> {
    let mut view = view(false)?;
    let now = anchor();
    assert!(view.activate(1));
    assert_eq!(view.appearance.reveal, 0.0);
    assert!(!view.appearance.status_visible());
    view.sample(StartupPhase::PreparingConnection, Duration::from_millis(110), now);
    assert_eq!(view.appearance.reveal, 0.5);
    let middle = view.appearance;
    assert!(!view.console(None).visible);
    view.sample(StartupPhase::PreparingConnection, Duration::from_millis(220), now);
    view.publish_controls(middle);
    assert_eq!(view.console(None).bounds.y, 458.0);
    assert_eq!(view.console(None).bounds.height, 75.0);
    view.publish_controls(view.appearance);
    assert_eq!(view.console(None).bounds.height, 162.0);
    view.activate(1);
    view.sample(StartupPhase::PreparingConnection, Duration::from_millis(330), now);
    assert_eq!(view.appearance.reveal, 0.5);
    view.activate(1);
    assert_eq!(view.appearance.reveal, 0.5);
    view.sample(StartupPhase::PreparingConnection, Duration::from_millis(440), now);
    assert_eq!(view.appearance.reveal, 0.75);
    view.activate(1);
    view.sample(StartupPhase::PreparingConnection, Duration::from_millis(660), now);
    view.publish_controls(view.appearance);
    assert!(!view.console(None).visible);
    assert!(view.appearance.status_visible());
    Ok(())
}

#[test]
fn reduced_motion_disclosure_is_immediate_and_mascot_is_static() -> Result<(), Box<dyn Error>> {
    let mut view = view(true)?;
    assert!(!view.pointer(PointerAction::Press, Pointer { x: 270.0, y: 200.0 }));
    view.activate(1);
    assert_eq!(view.appearance.reveal, 1.0);
    view.activate(1);
    assert_eq!(view.appearance.reveal, 0.0);
    assert!(view.appearance.status_visible());
    Ok(())
}
