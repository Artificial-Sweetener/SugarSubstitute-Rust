// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Assert real view state follows acknowledgements and shared control geometry.
use super::*;
use std::error::Error;
fn view(reduced: bool) -> Result<StartupView, Box<dyn Error>> {
    Ok(StartupView::new(
        Presentation::new("en", RequestedTheme::Dark, reduced)?,
        sugarsubstitute_ui::accent::AccentPalette::PRODUCT,
    ))
}

/// Every noninteractive splash region moves the same window; disclosure follows publication.
#[test]
fn splash_body_is_draggable_except_published_console_and_live_controls()
-> Result<(), Box<dyn Error>> {
    let mut view = view(true)?;
    for point in [
        Pointer { x: 270.0, y: 200.0 },
        Pointer { x: 270.0, y: 350.0 },
        Pointer { x: 270.0, y: 440.0 },
        Pointer { x: 10.0, y: 440.0 },
        Pointer { x: 550.0, y: 550.0 },
    ] {
        assert_eq!(view.hit_test(point), HitTarget::Caption);
    }
    view.activate(1);
    let output = Pointer { x: 270.0, y: 440.0 };
    assert_eq!(view.hit_test(output), HitTarget::Caption);
    view.publish_controls(view.appearance);
    assert_eq!(view.hit_test(output), HitTarget::Client);
    assert_eq!(view.hit_test(Pointer { x: 10.0, y: 440.0 }), HitTarget::Caption);
    assert_eq!(view.hit_test(Pointer { x: 270.0, y: 359.0 }), HitTarget::Caption);
    view.phase(StartupPhase::Stopped);
    assert_eq!(view.hit_test(Pointer { x: 270.0, y: 510.0 }), HitTarget::Client);
    assert_eq!(view.hit_test(Pointer { x: 480.0, y: 12.0 }), HitTarget::Client);
    assert_eq!(view.hit_test(Pointer { x: 530.0, y: 12.0 }), HitTarget::Close);
    view.activate(1);
    view.publish_controls(view.appearance);
    assert_eq!(view.hit_test(output), HitTarget::Caption);
    Ok(())
}
#[test]
fn static_waiting_feedback_wakes_at_each_boundary_without_continuous_animation()
-> Result<(), Box<dyn Error>> {
    let mut view = view(true)?;
    let origin = Instant::now();
    for (elapsed, next) in [(0, Some(120)), (119, Some(120)), (120, Some(300)), (300, None)] {
        let elapsed = Duration::from_secs(elapsed);
        view.sample(StartupPhase::PreparingConnection, elapsed, origin + elapsed);
        assert!(!view.motion());
        assert!(!view.feedback_due(origin + elapsed));
        assert_eq!(
            view.feedback_deadline(),
            next.map(|seconds| origin + Duration::from_secs(seconds))
        );
        if let Some(seconds) = next {
            let deadline = origin + Duration::from_secs(seconds);
            assert!(!view.feedback_due(deadline - Duration::from_nanos(1)));
            assert!(view.feedback_due(deadline));
        }
    }
    view.sample(StartupPhase::Stopped, Duration::from_secs(301), origin + Duration::from_secs(301));
    view.phase(StartupPhase::SplashFirstFrame);
    view.sample(
        StartupPhase::SplashFirstFrame,
        Duration::from_secs(3600),
        origin + Duration::from_secs(3600),
    );
    assert_eq!(view.feedback_deadline(), Some(origin + Duration::from_secs(3720)));
    assert_eq!(view.appearance.status, sugarsubstitute_splash::StartupStatus::Loading);
    view.sample(
        StartupPhase::Cancelling,
        Duration::from_secs(3601),
        origin + Duration::from_secs(3601),
    );
    assert_eq!(view.feedback_deadline(), None);
    Ok(())
}
#[test]
fn slow_startup_feedback_keeps_waiting_and_resets_only_for_a_settled_retry()
-> Result<(), Box<dyn Error>> {
    use sugarsubstitute_splash::StartupStatus;
    for reduced in [false, true] {
        let mut view = view(reduced)?;
        for (seconds, status) in [
            (119, StartupStatus::Loading),
            (120, StartupStatus::LoadingLong),
            (299, StartupStatus::LoadingLong),
            (300, StartupStatus::LoadingExtended),
            (7200, StartupStatus::LoadingExtended),
        ] {
            view.elapsed = Duration::from_secs(seconds);
            view.phase(StartupPhase::PreparingConnection);
            assert_eq!(view.appearance.status, status);
            assert_eq!(view.progress().completed, 0.08);
            assert!(!view.activate(3));
        }
        view.phase(StartupPhase::WebviewLoading);
        assert_eq!(view.appearance.status, StartupStatus::LoadingExtended);
        view.phase(StartupPhase::Cancelling);
        assert_eq!(view.appearance.status, StartupStatus::Settling);
        view.phase(StartupPhase::Stopped);
        view.phase(StartupPhase::SplashFirstFrame);
        assert_eq!(view.appearance.status, StartupStatus::Loading);
        view.elapsed += Duration::from_secs(120);
        view.phase(StartupPhase::PreparingConnection);
        assert_eq!(view.appearance.status, StartupStatus::LoadingLong);
    }
    Ok(())
}
#[test]
fn phase_feedback_is_one_estimate_and_silent_time_cannot_complete_it() -> Result<(), Box<dyn Error>>
{
    let mut view = view(true)?;
    view.phase(StartupPhase::PreparingConnection);
    assert_eq!(view.progress().completed, 0.08);
    view.elapsed = Duration::from_secs(7200);
    view.phase(StartupPhase::PreparingConnection);
    assert_eq!(view.progress().completed, 0.08);
    assert!(view.progress().activity.is_none());
    view.phase(StartupPhase::ServerReady);
    assert_eq!(view.progress().completed, 0.80);
    view.phase(StartupPhase::WebviewLoading);
    assert_eq!(view.progress().completed, 0.88);
    view.phase(StartupPhase::FrontendReady);
    assert_eq!(view.progress().completed, 0.97);
    view.phase(StartupPhase::Running);
    assert_eq!(view.progress().completed, 1.0);
    assert!(view.progress().activity.is_none());
    Ok(())
}
#[test]
fn terminal_phases_stop_activity_and_retain_progress_for_diagnosis() -> Result<(), Box<dyn Error>> {
    for reduced in [true, false] {
        let mut view = view(reduced)?;
        view.phase(StartupPhase::PreparingConnection);
        view.elapsed = Duration::from_secs(1);
        view.phase(StartupPhase::WebviewLoading);
        let completed = view.progress().completed;
        view.phase(StartupPhase::Failed);
        assert_eq!(view.progress().completed, completed);
        assert!(view.progress().activity.is_none());
        view.phase(StartupPhase::Stopped);
        view.phase(StartupPhase::SplashFirstFrame);
        assert_eq!(view.progress().completed, 0.0);
    }
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
    v.publish_controls(v.appearance);
    assert_ne!(v.controls()[0].label, before);
    assert!(!v.activate(2));
    assert!(v.controls()[1].close);
    Ok(())
}

#[test]
fn keyboard_focus_does_not_activate_disclosure_and_reduced_motion_stays_static()
-> Result<(), Box<dyn Error>> {
    let mut v = view(true)?;
    assert!(!v.motion());
    assert!(!v.key(KeyAction::Tab));
    assert!(!v.appearance.expanded);
    assert!(v.activate(1));
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

#[test]
fn native_console_visibility_tracks_published_disclosure_and_preserves_identity_across_collapse()
-> Result<(), Box<dyn Error>> {
    let mut view = view(true)?;
    assert!(!view.console(None).visible);
    assert!(view.activate(1));
    assert!(!view.console(None).visible);
    view.publish_controls(view.appearance);
    assert!(view.console(None).visible);
    let label = view.console(None).label;
    assert_eq!(label, "Comfy output");
    assert!(view.activate(1));
    assert!(view.console(None).visible);
    view.publish_controls(view.appearance);
    assert!(!view.console(None).visible);
    assert_eq!(view.console(None).label, label);
    assert!(view.activate(1));
    view.publish_controls(view.appearance);
    assert!(view.console(None).visible);
    Ok(())
}
#[test]
fn readonly_console_geometry_preserves_original_retry_peer_and_locale_owner()
-> Result<(), Box<dyn Error>> {
    for messages in sugarsubstitute_contracts::localization::LOCALES {
        let mut view = StartupView::new(
            Presentation::new(messages.id, RequestedTheme::Dark, true)?,
            sugarsubstitute_ui::accent::AccentPalette::PRODUCT,
        );
        view.activate(1);
        view.publish_controls(view.appearance);
        let before = view.console(None);
        assert_eq!(before.label, messages.output);
        assert!(before.visible);
        assert_eq!(
            (before.bounds.x, before.bounds.y, before.bounds.width, before.bounds.height),
            (25.0, 371.0, 508.0, 162.0)
        );
        view.phase(StartupPhase::Stopped);
        let console = view.console(None);
        assert!(console.bounds.y + console.bounds.height <= view.controls()[2].bounds.y);
        assert!(view.controls()[2].visible);
        assert!(view.activate(3));
    }
    Ok(())
}
