// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Pump a main-thread native lifetime without application windows, beside at most one
//! surface the lifetime asks for.
mod state;
#[cfg(target_os = "windows")]
mod surface;
use crate::{
    DesktopError, StartupPreparationStep,
    bootstrap::{Bootstrap, WaitOutcome},
    completion_wake::{ProxyWake, WakeLifetime},
    native_action::NativeAction,
};
use std::sync::Arc;
use tao::{
    event::Event,
    event_loop::{ControlFlow, EventLoopBuilder},
    platform::run_return::EventLoopExtRunReturn,
};

use crate::{HeadlessLifetime, HeadlessStep};

/// Run one main-thread native pump with no application window, frame or focus work.
///
/// When the lifetime asks for a surface through [`HeadlessLifetime::take_presentation`], the
/// pump hosts it as an unowned form on this same loop while the lifetime keeps working, and
/// closes it before the lifetime finishes. The loop runs until both have settled.
///
/// A private native event exists before `start`; preparation completes before Tao.
/// The same wake endpoint then retargets to the actual proxy under synchronization.
/// Each activity call receives budget 32. Active and
/// closing waits use the owner's earliest deadline or actual completion, keeping
/// the loop/proxy alive until actual settlement without periodic re-polling.
/// An early native return is an error and re-enters the same
/// retained Windows event loop to continue shutdown; it never fabricates success.
/// The wake disconnects and the native loop drops before final owner teardown.
///
/// # Errors
/// Return the first original startup/activity/cleanup/native-loop failure after
/// settlement; secondary causes reach structured logging with their operation.
/// This function must run on the native main thread. It performs no process launch, raw
/// worker scheduling or blocking owner join, and creates no window except the requested form.
pub fn run_headless(lifetime: &mut impl HeadlessLifetime) -> Result<(), DesktopError> {
    let bootstrap = Bootstrap::new()?;
    let mut state = state::HeadlessState::new();
    state.start(lifetime, bootstrap.wake());
    prepare_before_loop(&bootstrap, &mut state, lifetime);
    if state.settled() {
        state.retire_bootstrap(&bootstrap);
        return state.finish(lifetime, || {});
    }
    let mut builder = EventLoopBuilder::<NativeAction>::with_user_event();
    #[cfg(target_os = "windows")]
    let mut surface = surface::Surface::new(crate::file_dialog::Picker::new(&mut builder));
    #[cfg(not(target_os = "windows"))]
    let surface = NoSurface;
    let mut event_loop = match crate::bootstrap::construct_loop(move || builder.build()) {
        Ok(event_loop) => event_loop,
        Err(cause) => {
            state.fail(lifetime, cause);
            prepare_before_loop(&bootstrap, &mut state, lifetime);
            state.retire_bootstrap(&bootstrap);
            return state.finish(lifetime, || {});
        }
    };
    let wake_lifetime = WakeLifetime::new();
    let wake = Arc::new(ProxyWake::new(
        event_loop.create_proxy(),
        wake_lifetime.clone(),
        NativeAction::Completions,
    ));
    #[cfg(target_os = "windows")]
    let form_proxy = event_loop.create_proxy();
    if let Err(cause) = bootstrap.handoff(wake) {
        state.fail(lifetime, cause);
        prepare_before_loop(&bootstrap, &mut state, lifetime);
    }
    let session_end = headless_session_end();
    while !state.settled() || surface.open() {
        let exit_code = event_loop.run_return(|event, _target, control| {
            match event {
                Event::UserEvent(NativeAction::Completions) => state.activity(lifetime),
                Event::MainEventsCleared => {
                    if session_ending(session_end.as_ref()) {
                        lifetime.session_ending();
                    }
                    state.activity(lifetime);
                    state.poll_shutdown(lifetime);
                    #[cfg(target_os = "windows")]
                    surface.turn(lifetime, _target, &form_proxy, state.closing());
                }
                #[cfg(target_os = "windows")]
                other => {
                    surface.route(&other);
                }
                #[cfg(not(target_os = "windows"))]
                _ => {}
            }
            *control = match state.control(lifetime) {
                ControlFlow::Exit if surface.open() => ControlFlow::Wait,
                flow => flow,
            };
        });
        state.native_return(lifetime, exit_code);
    }
    state.retire_bootstrap(&bootstrap);
    if let Some(watch) = session_end
        && let Err(cause) = watch.retire()
    {
        tracing::error!(operation = "retire_session_end", cause = %cause,
            "The headless session-end watch could not be removed");
    }
    state.finish(lifetime, || {
        wake_lifetime.disconnect();
        drop(event_loop);
    })
}

/// Platforms without hosted forms never open a surface.
#[cfg(not(target_os = "windows"))]
struct NoSurface;

#[cfg(not(target_os = "windows"))]
impl NoSurface {
    const fn open(&self) -> bool {
        false
    }
}

/// Keep Tao from exiting a headless process from inside its window procedure at session end.
/// A headless role holds nothing open for the user, so it never delays the session end.
#[cfg(target_os = "windows")]
fn headless_session_end() -> Option<crate::windows_session_end::SessionEndWatch> {
    match crate::windows_session_end::SessionEndWatch::install("", || {}) {
        Ok(watch) => {
            watch.set_outstanding(false);
            Some(watch)
        }
        Err(cause) => {
            tracing::error!(operation = "install_session_end", cause = %cause,
                "A headless session end will exit without its owned shutdown");
            None
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn headless_session_end() -> Option<NoSessionEnd> {
    None
}

/// Take a waiting signal; only a session that is actually ending is reported.
#[cfg(target_os = "windows")]
fn session_ending(watch: Option<&crate::windows_session_end::SessionEndWatch>) -> bool {
    matches!(
        watch.and_then(crate::windows_session_end::SessionEndWatch::take_signal),
        Some(crate::windows_session_end::SessionEndSignal::Ending { .. })
    )
}

#[cfg(not(target_os = "windows"))]
fn session_ending(_: Option<&NoSessionEnd>) -> bool {
    false
}

/// Platforms without Tao's session-end exit need no watch.
#[cfg(not(target_os = "windows"))]
struct NoSessionEnd;

#[cfg(not(target_os = "windows"))]
impl NoSessionEnd {
    fn retire(self) -> Result<(), DesktopError> {
        Ok(())
    }
}

/// Closing keeps the same original lifetime, with at most 32 owner/message notices per turn.
fn prepare_before_loop(
    bootstrap: &Bootstrap,
    state: &mut state::HeadlessState,
    lifetime: &mut impl HeadlessLifetime,
) {
    while !state.prepared() && !state.settled() {
        if let Err(cause) = bootstrap.begin_turn() {
            state.fail(lifetime, cause);
        }
        state.prepare(lifetime);
        if state.prepared() || state.settled() {
            break;
        }
        let deadline = match state.control(lifetime) {
            tao::event_loop::ControlFlow::WaitUntil(deadline) => Some(deadline),
            _ => None,
        };
        match bootstrap.wait(deadline) {
            Ok(WaitOutcome::Quit(status)) => state.native_return(lifetime, status),
            Ok(WaitOutcome::Woken | WaitOutcome::Deadline) => {}
            Err(cause) => state.fail(lifetime, cause),
        }
    }
}
