// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prepare actual content before Tao, then retain existing teardown on the native loop.

#[path = "startup_preparation_state.rs"]
mod state;
use crate::{
    DesktopError, StartupWindowContent,
    bootstrap::{Bootstrap, WaitOutcome},
    native_action::NativeAction,
};
pub(super) use state::PreparationExit;
use state::PreparationState;
use std::{num::NonZeroUsize, sync::Arc, time::Instant};
use sugarsubstitute_contracts::execution::CompletionWake;
use tao::{
    event::Event,
    event_loop::{ControlFlow, EventLoop},
    platform::run_return::EventLoopExtRunReturn,
};

/// Start once and retain actual prerequisites before creating any Tao event loop.
pub(super) fn prepare(
    bootstrap: &Bootstrap,
    content: &mut impl StartupWindowContent,
    wake: Arc<dyn CompletionWake>,
    maximum: NonZeroUsize,
    limit: Option<Instant>,
    timings: Option<&crate::timing::TimingObserver>,
) -> (PreparationExit, Option<DesktopError>) {
    let mut state = PreparationState::new();
    if state.start(content, wake)
        && let Err(cause) = crate::timing::observe_startup(
            timings,
            crate::timing::NativeStartupStage::ExecutionReady,
        )
    {
        state.fail(content, cause);
    }
    drive_before_loop(bootstrap, content, maximum, limit, state)
}

/// Retain already started content after failed loop construction or wake handoff.
pub(super) fn close_before_loop(
    bootstrap: &Bootstrap,
    content: &mut impl StartupWindowContent,
    maximum: NonZeroUsize,
    limit: Option<Instant>,
    cause: DesktopError,
) -> Option<DesktopError> {
    let state = PreparationState::closing_started(content, cause);
    drive_before_loop(bootstrap, content, maximum, limit, state).1
}
fn drive_before_loop(
    bootstrap: &Bootstrap,
    content: &mut impl StartupWindowContent,
    maximum: NonZeroUsize,
    limit: Option<Instant>,
    mut state: PreparationState,
) -> (PreparationExit, Option<DesktopError>) {
    loop {
        if let Err(cause) = bootstrap.begin_turn() {
            state.fail(content, cause);
        }
        if limit.is_some_and(|deadline| Instant::now() >= deadline) {
            state.close(content);
        }
        state.activity(content, maximum);
        if let Some(exit) = state.exit() {
            return (exit, state.take_failure());
        }
        let deadline =
            match preparation_wait(state.closing(), content.next_deadline(), limit, Instant::now())
            {
                ControlFlow::WaitUntil(deadline) => Some(deadline),
                _ => None,
            };
        match bootstrap.wait(deadline) {
            Ok(WaitOutcome::Quit(status)) => {
                state.native_return(content, status);
            }
            Ok(WaitOutcome::Woken | WaitOutcome::Deadline) => {}
            Err(cause) => state.fail(content, cause),
        }
    }
}

/// Retire already started content after a failed STA boundary without starting it again.
pub(super) fn close_before_surface(
    event_loop: &mut EventLoop<NativeAction>,
    content: &mut impl StartupWindowContent,
    maximum: NonZeroUsize,
    limit: Option<Instant>,
    cause: DesktopError,
) -> Option<DesktopError> {
    let state = PreparationState::closing_started(content, cause);
    drive(event_loop, content, maximum, limit, state).1
}

fn drive(
    event_loop: &mut EventLoop<NativeAction>,
    content: &mut impl StartupWindowContent,
    maximum: NonZeroUsize,
    limit: Option<Instant>,
    mut state: PreparationState,
) -> (PreparationExit, Option<DesktopError>) {
    loop {
        let status = event_loop.run_return(|event, _, control| {
            if limit.is_some_and(|deadline| Instant::now() >= deadline) {
                state.close(content);
            }
            match event {
                Event::UserEvent(NativeAction::Completions) | Event::MainEventsCleared => {
                    state.activity(content, maximum);
                }
                Event::UserEvent(NativeAction::Close) => state.close(content),
                _ => {}
            }
            *control = if state.exit().is_some() {
                ControlFlow::Exit
            } else {
                preparation_wait(state.closing(), content.next_deadline(), limit, Instant::now())
            };
        });
        if let Some(exit) = state.native_return(content, status) {
            return (exit, state.take_failure());
        }
    }
}

/// Use existing native closing cadence and the owner's absolute deadline without rebasing.
fn preparation_wait(
    closing: bool,
    application: Option<Instant>,
    limit: Option<Instant>,
    now: Instant,
) -> ControlFlow {
    let deadline = match (application, limit) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    };
    if closing {
        match (crate::cadence::closing_wait(now), deadline.filter(|deadline| *deadline > now)) {
            (ControlFlow::WaitUntil(native), Some(deadline)) => {
                ControlFlow::WaitUntil(native.min(deadline))
            }
            (native, _) => native,
        }
    } else {
        deadline.map_or(ControlFlow::Wait, ControlFlow::WaitUntil)
    }
}

#[cfg(test)]
#[path = "startup_preparation_tests.rs"]
mod tests;
