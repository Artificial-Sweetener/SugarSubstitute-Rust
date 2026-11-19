// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify actual placement and peer retention with only the frontend readiness boundary faked.

use super::*;
use crate::{MainCaptionLabels, MainWindowRequest, MainWindowSettings, ServedFrontendVersions};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use sugarsubstitute_contracts::{
    configuration::{LoopbackOrigin, ProfileId},
    execution::{Generation, MemoryLease, MemoryRelease},
    lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey},
    startup::LaunchId,
};
use tao::{
    event::Event,
    event_loop::{ControlFlow, EventLoop, EventLoopBuilder},
    platform::{run_return::EventLoopExtRunReturn, windows::EventLoopBuilderExtWindows},
};
use windows::Win32::{
    Foundation::{LPARAM, WPARAM},
    UI::WindowsAndMessaging::{GetWindowRect, PostMessageW, WM_CLOSE},
};

type TestResult = Result<(), Box<dyn Error>>;

struct Released(Arc<AtomicU64>);
impl MemoryRelease for Released {
    fn release(&self, bytes: u64) {
        self.0.fetch_add(bytes, Ordering::SeqCst);
    }
}

#[test]
fn placement_native_hidden_restore_and_final_capture_retain_exact_peer() -> TestResult {
    crate::windows_test_desktop::run(
        concat!(
            module_path!(),
            "::placement_native_hidden_restore_and_final_capture_retain_exact_peer"
        ),
        qualify_placement,
    )
}

/// Drive production construction/capture/retirement; only the frontend-ready receipt is simulated.
fn qualify_placement() -> TestResult {
    let _apartment = crate::windows_ui_apartment::UiApartment::enter()?;
    let mut builder = EventLoopBuilder::<crate::native_action::NativeAction>::with_user_event();
    let mut events = builder.with_any_thread(true).build();
    let primary = events.primary_monitor().ok_or("native primary monitor absent")?;
    let (monitor, _) = monitor_facts(HMONITOR(primary.hmonitor() as *mut _))?;
    let scale = monitor.dpi / 96.0;
    let bounds = Rect {
        x: monitor.work.x + 50.0 * scale,
        y: monitor.work.y + 60.0 * scale,
        width: 680.0 * scale,
        height: 460.0 * scale,
    };
    let identity = first_identity()?;
    for maximized in [false, true] {
        let saved = main_placement::snapshot(bounds, maximized, &monitor)?;
        let expected = main_placement::restore(&saved, std::slice::from_ref(&monitor))?;
        let released = Arc::new(AtomicU64::new(0));
        let mut request = main_request(identity, &released)?;
        request.set_initial_placement(Some(saved))?;
        let mut actor = MainActor::new(request, events.create_proxy(), false)?;
        let result = (|| -> TestResult {
            assert!(
                actor
                    .construct_step(&events, events.create_proxy(), "Placement qualification")?
                    .is_none()
            );
            assert!(
                actor
                    .construct_step(&events, events.create_proxy(), "Placement qualification")?
                    .is_none()
            );
            assert!(!actor.window()?.is_visible(), "construction must not reveal the native peer");
            let mut physical = RECT::default();
            // SAFETY: The actor retains this hidden peer on the creating UI thread.
            unsafe { GetWindowRect(hwnd(actor.window()?), &raw mut physical) }?;
            assert_eq!(
                Rect {
                    x: f64::from(physical.left),
                    y: f64::from(physical.top),
                    width: f64::from(physical.right - physical.left),
                    height: f64::from(physical.bottom - physical.top),
                },
                expected.bounds,
                "hidden native rectangle applies before children/reveal"
            );
            let capture_id = NonZeroU64::new(1).ok_or("invalid capture identity")?;
            assert!(matches!(
                actor.capture_placement(identity, capture_id),
                DesktopStartupObservation::MainPlacementCaptureFailed { .. }
            ));
            // The fake frontend boundary supplies readiness only. Native visibility, restored
            // maximize state, captures, close intent and peer lifetime are actual production APIs.
            actor.show_placed_window()?;
            actor.revealed = true;
            assert!(actor.window()?.is_visible());
            drain_native(&mut events)?;
            let visible = capture(actor.window()?)?;
            assert_eq!(visible.maximized, maximized);
            // Before reveal the current HWND geometry is authoritative. First show must
            // preserve those bounds as the normal rectangle even when maximized.
            assert_eq!(
                Rect {
                    x: visible.bounds.x * scale,
                    y: visible.bounds.y * scale,
                    width: visible.bounds.width * scale,
                    height: visible.bounds.height * scale,
                },
                expected.bounds,
                "reveal/maximize preserves the restored normal rectangle; maximized={maximized}"
            );
            actor.window()?.set_minimized(true);
            let minimized = capture(actor.window()?)?;
            assert_eq!(minimized.bounds, visible.bounds);
            assert_eq!(minimized.maximized, maximized, "next launch never restores minimized");
            actor.window()?.set_minimized(false);
            close_intent(&mut actor, &mut events)?;
            assert!(
                !actor.admission.closing(),
                "close intent must retain the native peer for save"
            );
            let observed = actor.capture_placement(identity, capture_id);
            assert!(matches!(observed, DesktopStartupObservation::MainPlacementObserved(ref value)
                if value.identity == identity && value.capture_id == Some(capture_id)));
            let foreign = MainIdentity { navigation: Generation::new(2)?, ..identity };
            assert!(
                matches!(actor.capture_placement(foreign, NonZeroU64::new(2).ok_or("invalid capture identity")?),
                DesktopStartupObservation::MainPlacementCaptureFailed { identity: failed, .. } if failed == foreign)
            );
            assert!(actor.window.is_some(), "failed capture must retain the usable peer");
            Ok(())
        })();
        let retirement = retire(&mut actor);
        drop(actor);
        result?;
        retirement?;
        assert_eq!(released.load(Ordering::SeqCst), MainWindowRequest::RESERVATION_BYTES);
    }
    Ok(())
}

/// A main window request whose profile and origin are never opened by these cases.
fn main_request(
    identity: MainIdentity,
    released: &Arc<AtomicU64>,
) -> Result<MainWindowRequest, Box<dyn Error>> {
    Ok(MainWindowRequest::new(
        identity,
        LoopbackOrigin::parse("http://127.0.0.1:8188/", 8188)?,
        ProfileId::new("placement-qualification".into())?,
        std::path::PathBuf::from(r"D:\placement-profile-not-opened"),
        ServedFrontendVersions::new("1.47.11".into(), "0.15.0".into())?,
        MainWindowSettings {
            logical_width: 1280,
            logical_height: 800,
            dark: true,
            labels: MainCaptionLabels {
                minimize: "Minimize",
                maximize: "Maximize",
                restore: "Restore",
                close: "Close",
            },
        },
        MemoryLease::new(
            MainWindowRequest::RESERVATION_BYTES,
            Box::new(Released(released.clone())),
        ),
    )?)
}

fn first_identity() -> Result<MainIdentity, Box<dyn Error>> {
    Ok(MainIdentity {
        run: RunKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([1; 32]),
                epoch: CoordinatorEpoch::new([2; 16])?,
            },
            instance: InstanceId::new(1)?,
            generation: RunGeneration::new(1)?,
        },
        launch: LaunchId::new(1)?,
        navigation: Generation::new(1)?,
    })
}

#[test]
fn a_replacement_main_shows_its_frame_before_its_page_and_holds_its_buttons_until_reveal()
-> TestResult {
    crate::windows_test_desktop::run(
        concat!(
            module_path!(),
            "::a_replacement_main_shows_its_frame_before_its_page_and_holds_its_buttons_until_reveal"
        ),
        qualify_replacement_frame,
    )
}

/// Drive production construction through the caption stage for a first and a replacement main.
fn qualify_replacement_frame() -> TestResult {
    let _apartment = crate::windows_ui_apartment::UiApartment::enter()?;
    let mut builder = EventLoopBuilder::<crate::native_action::NativeAction>::with_user_event();
    let events = builder.with_any_thread(true).build();
    for replacement in [false, true] {
        let released = Arc::new(AtomicU64::new(0));
        let request = main_request(first_identity()?, &released)?;
        let mut actor = MainActor::new(request, events.create_proxy(), replacement)?;
        let result = (|| -> TestResult {
            // Window, presenter and caption controls; the browser is never begun.
            for _ in 0..3 {
                assert!(
                    actor.construct_step(&events, events.create_proxy(), "Reload frame")?.is_none()
                );
            }
            let window = actor.window()?;
            assert_eq!(window.is_visible(), replacement, "replacement={replacement}");
            // The frame's close and caption buttons wait until its page can answer.
            let close = tao::event::WindowEvent::CloseRequested;
            if replacement {
                assert!(actor.window_event(window.id(), &close)?.is_none());
                assert!(actor.activate(3)?.is_none());
                assert!(actor.activate(2)?.is_none());
                assert!(!actor.window()?.is_maximized());
            } else {
                assert!(actor.window_event(window.id(), &close).is_err());
            }
            Ok(())
        })();
        let retirement = retire(&mut actor);
        drop(actor);
        result?;
        retirement?;
        assert_eq!(released.load(Ordering::SeqCst), MainWindowRequest::RESERVATION_BYTES);
    }
    Ok(())
}

#[test]
fn a_reloading_main_keeps_its_frame_on_screen_until_its_replacement_shows_one() -> TestResult {
    crate::windows_test_desktop::run(
        concat!(
            module_path!(),
            "::a_reloading_main_keeps_its_frame_on_screen_until_its_replacement_shows_one"
        ),
        qualify_frame_handoff,
    )
}

/// Retire a revealed main for its own reload, then build its replacement, with real windows.
fn qualify_frame_handoff() -> TestResult {
    use windows::Win32::UI::WindowsAndMessaging::{IsWindow, IsWindowVisible};
    let _apartment = crate::windows_ui_apartment::UiApartment::enter()?;
    let mut builder = EventLoopBuilder::<crate::native_action::NativeAction>::with_user_event();
    let events = builder.with_any_thread(true).build();
    let released = Arc::new(AtomicU64::new(0));
    let first_identity = first_identity()?;
    let mut first =
        MainActor::new(main_request(first_identity, &released)?, events.create_proxy(), false)?;
    let mut second = None;
    let result = (|| -> TestResult {
        for _ in 0..3 {
            assert!(first.construct_step(&events, events.create_proxy(), "Reload")?.is_none());
        }
        first.show_placed_window()?;
        first.revealed = true;
        let old = hwnd(first.window()?);
        assert!(first.reload.request(true, crate::MainReloadReason::Navigation).is_some());
        retire(&mut first)?;
        assert!(first.retired() && !first.settled(), "the frame outlives the retired lifetime");
        // SAFETY: Queries of a window this thread created; a destroyed handle answers false.
        assert!(unsafe { IsWindow(Some(old)).as_bool() && IsWindowVisible(old).as_bool() });

        let next = MainIdentity {
            launch: LaunchId::new(2)?,
            navigation: Generation::new(2)?,
            ..first_identity
        };
        let replacement = second.insert(MainActor::new(
            main_request(next, &released)?,
            events.create_proxy(),
            true,
        )?);
        replacement.inherit_frame(first.take_kept_frame());
        assert!(first.settled());
        // The old frame stays until the replacement's own frame is on screen.
        for _ in 0..2 {
            assert!(replacement.progress(&events, events.create_proxy(), "Reload")?.is_none());
            // SAFETY: As above.
            assert!(unsafe { IsWindowVisible(old).as_bool() });
        }
        let mut turns = 0;
        while replacement.inherited_frame.is_some() {
            assert!(replacement.progress(&events, events.create_proxy(), "Reload")?.is_none());
            turns += 1;
            assert!(turns < 16, "the inherited frame never retired");
        }
        assert!(replacement.window()?.is_visible());
        // SAFETY: As above.
        assert!(!unsafe { IsWindow(Some(old)).as_bool() });
        Ok(())
    })();
    let mut retirement = Ok(());
    if !first.retired() {
        retirement = retire(&mut first);
    }
    while !first.abandon_kept_frame()? {}
    if let Some(mut replacement) = second {
        let outcome = retire(&mut replacement);
        if retirement.is_ok() {
            retirement = outcome;
        }
    }
    drop(first);
    result?;
    retirement?;
    assert_eq!(released.load(Ordering::SeqCst), 2 * MainWindowRequest::RESERVATION_BYTES);
    Ok(())
}

/// Dispatch queued native construction messages before sampling the live OS placement.
fn drain_native(events: &mut EventLoop<crate::native_action::NativeAction>) -> TestResult {
    let exit = events.run_return(|event, _, control| {
        *control = if matches!(event, Event::MainEventsCleared) {
            ControlFlow::Exit
        } else {
            ControlFlow::Poll
        };
    });
    assert_eq!(exit, 0, "native construction dispatch exited unsuccessfully");
    Ok(())
}

/// Dispatch actual WM_CLOSE on this fixture's retained event loop until Tao delivers it.
///
/// The message is already queued on this thread, so the loop waits on the OS event itself and
/// exits on delivery; it has no wall-clock cutoff for load to turn into a failure.
fn close_intent(
    actor: &mut MainActor,
    events: &mut EventLoop<crate::native_action::NativeAction>,
) -> TestResult {
    let target = actor.window()?.id();
    let handle = hwnd(actor.window()?);
    // SAFETY: This test retains its exact native peer on the creating thread; the message is
    // inert close intent consumed by Tao, not destruction or input on another window/desktop.
    unsafe { PostMessageW(Some(handle), WM_CLOSE, WPARAM(0), LPARAM(0)) }?;
    let mut observed = None;
    let exit_code = events.run_return(|event, _, control| {
        if let Event::WindowEvent {
            window_id, event: tao::event::WindowEvent::CloseRequested, ..
        } = event
            && window_id == target
        {
            observed =
                Some(actor.window_event(window_id, &tao::event::WindowEvent::CloseRequested));
        }
        *control = if observed.is_some() { ControlFlow::Exit } else { ControlFlow::Wait };
    });
    assert_eq!(exit_code, 0, "native close dispatch exited unsuccessfully");
    assert!(matches!(
        observed.ok_or("native close dispatch ended without the close intent")??,
        Some(DesktopStartupObservation::CloseRequested)
    ));
    Ok(())
}

/// Advance the finite set of owned native stages; no wait, sleep, worker or detached cleanup.
fn retire(actor: &mut MainActor) -> TestResult {
    actor.close(actor.identity())?;
    for _ in 0..16 {
        if matches!(actor.retire_step()?, Some(DesktopStartupObservation::MainRetired(_))) {
            assert!(actor.window.is_none());
            return Ok(());
        }
    }
    Err("placement qualification did not retire its owned native stages".into())
}
