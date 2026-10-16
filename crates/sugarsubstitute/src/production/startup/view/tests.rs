// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Assert real view state follows acknowledgements and shared control geometry.
use super::*;
use std::error::Error;
fn view(reduced: bool) -> Result<StartupView, Box<dyn Error>> {
    Ok(StartupView::new(Presentation::new("en", RequestedTheme::Dark, reduced)?))
}
#[test]
fn elapsed_time_changes_activity_only_and_cannot_publish_completed_startup()
-> Result<(), Box<dyn Error>> {
    let mut view = view(false)?;
    for elapsed in [Duration::ZERO, Duration::from_secs(1), Duration::from_secs(3600)] {
        view.elapsed = elapsed;
        assert_eq!(view.progress(StartupPhase::PreparingConnection).completed, 0.0);
    }
    assert_eq!(view.progress(StartupPhase::ServerReady).completed, 0.5);
    assert_eq!(view.progress(StartupPhase::WebviewLoading).completed, 0.5);
    assert_eq!(view.progress(StartupPhase::FrontendReady).completed, 1.0);
    Ok(())
}
#[test]
fn reduced_motion_and_terminal_phases_stop_activity_without_changing_acknowledged_progress()
-> Result<(), Box<dyn Error>> {
    assert!(view(true)?.progress(StartupPhase::PreparingConnection).activity.is_none());
    let view = view(false)?;
    for phase in [
        StartupPhase::SplashFirstFrame,
        StartupPhase::Running,
        StartupPhase::Failed,
        StartupPhase::Cancelling,
        StartupPhase::Stopped,
    ] {
        assert!(view.progress(phase).activity.is_none());
    }
    assert_eq!(view.progress(StartupPhase::Running).completed, 1.0);
    Ok(())
}
#[test]
fn pointer_disclosure_requires_press_and_release_on_same_owned_control()
-> Result<(), Box<dyn Error>> {
    let mut v = view(true)?;
    let toggle = Pointer { x: 480.0, y: 12.0 };
    v.pointer(PointerAction::Press, toggle);
    v.pointer(PointerAction::Release, Pointer { x: 500.0, y: 80.0 });
    assert!(!v.appearance.expanded);
    v.pointer(PointerAction::Press, toggle);
    v.pointer(PointerAction::Release, toggle);
    assert!(v.appearance.expanded);
    assert_eq!(v.hit_test(toggle), HitTarget::Client);
    assert_eq!(v.hit_test(Pointer { x: 530.0, y: 12.0 }), HitTarget::Close);
    Ok(())
}
#[test]
fn accessible_disclosure_label_waits_for_matching_frame_publication() -> Result<(), Box<dyn Error>>
{
    let mut v = view(true)?;
    let before = v.controls()[0].label;
    assert!(v.activate(1));
    assert_eq!(v.controls()[0].label, before);
    v.publish_controls();
    assert_ne!(v.controls()[0].label, before);
    assert!(!v.activate(2));
    assert!(v.controls()[1].close);
    Ok(())
}

#[test]
fn keyboard_and_reduced_motion_use_the_same_actual_disclosure_and_animation()
-> Result<(), Box<dyn Error>> {
    let mut v = view(true)?;
    assert!(!v.motion());
    assert!(v.key(KeyAction::Tab));
    assert!(v.appearance.expanded);
    let original = v.animation.frame(Duration::ZERO);
    v.animation.tick(Duration::from_secs(3600));
    assert_eq!(v.animation.frame(Duration::from_secs(3600)), original);
    assert!(view(false)?.motion());
    Ok(())
}

#[test]
fn recovery_control_is_localized_and_inert_until_policy_observes_settlement()
-> Result<(), Box<dyn Error>> {
    let mut view = view(true)?;
    let retry = Pointer { x: 270.0, y: 510.0 };
    for phase in [StartupPhase::PreparingConnection, StartupPhase::Failed, StartupPhase::Cancelling]
    {
        view.phase(phase);
        assert_eq!(view.controls().len(), 3);
        assert_eq!(view.controls().iter().map(|control| control.id).collect::<Vec<_>>(), [1, 2, 3]);
        assert!(!view.controls()[2].visible);
        assert!(!view.activate(3));
        view.pointer(PointerAction::Press, retry);
        view.pointer(PointerAction::Release, retry);
        assert!(!view.take_retry());
    }
    view.phase(StartupPhase::Stopped);
    assert_eq!(view.controls()[2].label, "Retry");
    assert!(view.controls()[2].visible);
    assert!(!view.controls()[2].close);
    view.pointer(PointerAction::Press, retry);
    view.pointer(PointerAction::Release, retry);
    assert!(view.take_retry());
    assert!(!view.take_retry());
    view.phase(StartupPhase::SplashFirstFrame);
    assert_eq!(view.controls().len(), 3);
    assert!(!view.controls()[2].visible);
    assert!(!view.activate(3));
    Ok(())
}
