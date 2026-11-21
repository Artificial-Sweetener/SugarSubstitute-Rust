// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Assert real view state follows acknowledgements and shared control geometry.
use super::*;
use std::error::Error;
use sugarsubstitute_execution::qualification_time::anchor;
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
    view.observe(StartupPhase::Stopped, None);
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
    let origin = anchor();
    for (elapsed, next) in [(0, Some(120)), (119, Some(120)), (120, Some(300)), (300, None)] {
        let elapsed = Duration::from_secs(elapsed);
        view.sample(StartupPhase::PreparingConnection, None, elapsed, origin + elapsed);
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
    view.sample(
        StartupPhase::Stopped,
        None,
        Duration::from_secs(301),
        origin + Duration::from_secs(301),
    );
    view.observe(StartupPhase::SplashFirstFrame, None);
    view.sample(
        StartupPhase::SplashFirstFrame,
        None,
        Duration::from_secs(3600),
        origin + Duration::from_secs(3600),
    );
    assert_eq!(view.feedback_deadline(), Some(origin + Duration::from_secs(3720)));
    assert_eq!(view.appearance.status, sugarsubstitute_splash::StartupStatus::Loading);
    view.sample(
        StartupPhase::Cancelling,
        None,
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
            view.observe(StartupPhase::PreparingConnection, None);
            assert_eq!(view.appearance.status, status);
            assert_eq!(view.progress().completed, 0.08);
            assert!(!view.activate(3));
        }
        view.observe(StartupPhase::WebviewLoading, None);
        assert_eq!(view.appearance.status, StartupStatus::LoadingExtended);
        view.observe(StartupPhase::Cancelling, None);
        assert_eq!(view.appearance.status, StartupStatus::Settling);
        view.observe(StartupPhase::Stopped, None);
        view.observe(StartupPhase::SplashFirstFrame, None);
        assert_eq!(view.appearance.status, StartupStatus::Loading);
        view.elapsed += Duration::from_secs(120);
        view.observe(StartupPhase::PreparingConnection, None);
        assert_eq!(view.appearance.status, StartupStatus::LoadingLong);
    }
    Ok(())
}
#[test]
fn phase_feedback_is_one_estimate_and_silent_time_cannot_complete_it() -> Result<(), Box<dyn Error>>
{
    let mut view = view(true)?;
    view.observe(StartupPhase::PreparingConnection, None);
    assert_eq!(view.progress().completed, 0.08);
    view.elapsed = Duration::from_secs(7200);
    view.observe(StartupPhase::PreparingConnection, None);
    assert_eq!(view.progress().completed, 0.08);
    assert!(view.progress().activity.is_none());
    view.observe(StartupPhase::ServerReady, None);
    assert_eq!(view.progress().completed, 0.80);
    view.observe(StartupPhase::WebviewLoading, None);
    assert_eq!(view.progress().completed, 0.88);
    view.observe(StartupPhase::FrontendReady, None);
    assert_eq!(view.progress().completed, 0.97);
    view.observe(StartupPhase::Running, None);
    assert_eq!(view.progress().completed, 1.0);
    assert!(view.progress().activity.is_none());
    Ok(())
}
#[test]
fn terminal_phases_stop_activity_and_retain_progress_for_diagnosis() -> Result<(), Box<dyn Error>> {
    for reduced in [true, false] {
        let mut view = view(reduced)?;
        view.observe(StartupPhase::PreparingConnection, None);
        view.elapsed = Duration::from_secs(1);
        view.observe(StartupPhase::WebviewLoading, None);
        let completed = view.progress().completed;
        view.observe(StartupPhase::Failed, None);
        assert_eq!(view.progress().completed, completed);
        assert!(view.progress().activity.is_none());
        view.observe(StartupPhase::Stopped, None);
        view.observe(StartupPhase::SplashFirstFrame, None);
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

/// The splash never asks anyone to retry: no control exists for it and a click on the status
/// line does nothing. A settled failed start is tried again by itself at once, five times in a
/// row; then the failure is asked about in the dialog, whose Retry starts the count again.
#[test]
fn a_settled_failure_is_retried_five_times_by_itself_then_asked_about() -> Result<(), Box<dyn Error>>
{
    let mut view = view(true)?;
    let status_line = Pointer { x: 270.0, y: 510.0 };
    assert_eq!(view.controls().iter().map(|control| control.id).collect::<Vec<_>>(), [1, 2]);
    for phase in [StartupPhase::PreparingConnection, StartupPhase::Failed, StartupPhase::Cancelling]
    {
        view.observe(phase, None);
        assert!(!view.take_retry());
        assert!(!view.asks_about_failure());
    }
    for _ in 0..2 {
        for _ in 0..5 {
            view.observe(StartupPhase::Stopped, None);
            view.pointer(PointerAction::Press, status_line);
            view.pointer(PointerAction::Release, status_line);
            assert!(!view.activate(3));
            assert!(view.appearance.retrying);
            assert!(!view.asks_about_failure());
            assert!(view.take_retry());
            assert!(!view.take_retry());
            view.begin_retry();
        }
        view.observe(StartupPhase::Stopped, None);
        assert!(!view.take_retry());
        assert!(!view.appearance.retrying);
        assert!(view.asks_about_failure());
        assert!(view.appearance.retries_by_itself());
        view.reset_retries();
    }
    Ok(())
}

/// A failure retrying cannot fix is asked about at once, never tried again by itself, even when
/// the start fails and settles before any frame is drawn in between.
#[test]
fn a_failure_retrying_cannot_fix_is_asked_about_at_once() -> Result<(), Box<dyn Error>> {
    use sugarsubstitute_contracts::startup::StartupFailureCode;
    for code in [StartupFailureCode::FrontendMismatch, StartupFailureCode::SugarCubesMissing] {
        let mut view = view(true)?;
        view.observe(StartupPhase::PreparingConnection, None);
        view.observe(StartupPhase::Failed, Some(code));
        assert!(!view.take_retry());
        view.observe(StartupPhase::Stopped, Some(code));
        assert_eq!(view.appearance.failure, Some(code));
        assert!(!view.take_retry());
        assert!(view.asks_about_failure());
        assert!(!view.appearance.retries_by_itself());
    }
    Ok(())
}

/// An attached ComfyUI writes no output to the application, so its splash offers none.
#[test]
fn an_attached_start_offers_no_output_disclosure() -> Result<(), Box<dyn Error>> {
    let mut view = view(true)?;
    view.set_output_available(false);
    assert!(!view.controls()[0].visible);
    assert!(!view.activate(1));
    view.pointer(PointerAction::Press, Pointer { x: 480.0, y: 10.0 });
    view.pointer(PointerAction::Release, Pointer { x: 480.0, y: 10.0 });
    assert!(!view.appearance.expanded);
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
fn readonly_console_geometry_keeps_its_place_and_locale_owner() -> Result<(), Box<dyn Error>> {
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
        view.observe(StartupPhase::Stopped, None);
        let console = view.console(None);
        assert_eq!(
            (console.bounds.x, console.bounds.y, console.bounds.width, console.bounds.height),
            (25.0, 371.0, 508.0, 162.0)
        );
    }
    Ok(())
}

/// While the previous SugarCubes version is put back no retry is due; once the retry line
/// returns, the retry is due at once.
#[test]
fn the_restoring_line_withholds_retry_until_the_retry_line_returns() -> Result<(), Box<dyn Error>> {
    let mut view = view(true)?;
    view.observe(StartupPhase::Stopped, None);
    view.show_status(sugarsubstitute_splash::StartupStatus::RestoringSugarcubes);
    assert!(!view.take_retry());
    view.show_status(sugarsubstitute_splash::StartupStatus::SugarcubesRetryPrevious);
    assert!(view.take_retry());
    Ok(())
}
