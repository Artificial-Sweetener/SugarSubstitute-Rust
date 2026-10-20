// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Dispatch bounded application commands and native observations through existing owners.

#[path = "startup_commands.rs"]
mod commands;
#[path = "startup_presentation.rs"]
mod presentation;
use super::*;
use crate::{
    cadence::{advance_deadline, closing_wait, presentation_wait},
    native_surface::input,
};
use sugarsubstitute_contracts::startup::{StartupFailure, StartupFailureCode};
use tao::{
    event::Event,
    event_loop::{EventLoopProxy, EventLoopWindowTarget},
};

pub(super) struct Context<'a> {
    pub(super) title: &'a str,
    pub(super) timings: Option<&'a crate::timing::TimingObserver>,
    pub(super) limit: Option<Instant>,
}

pub(super) fn dispatch(
    session: &mut Session,
    content: &mut impl StartupWindowContent,
    event: Event<'_, NativeAction>,
    target: &EventLoopWindowTarget<NativeAction>,
    proxy: EventLoopProxy<NativeAction>,
    context: &Context<'_>,
) -> Result<(), DesktopError> {
    if matches!(event, Event::UserEvent(NativeAction::Activity)) {
        session.observers.consumed_wake();
    }
    session.activity.bind(content.run_key())?;
    if context.limit.is_some_and(|deadline| Instant::now() >= deadline) {
        session.close(content);
    }
    session.splash.bind_launch(content.launch_id());
    activity(session, content)?;
    let shape = session.splash.shape()?;
    if !session.splash.retiring {
        let visible = session.splash.window.as_ref().is_some_and(|window| {
            (window.is_visible()
                || session
                    .splash
                    .presentations
                    .as_ref()
                    .is_some_and(|owner| owner.preparing_first()))
                && !window.is_minimized()
        }) && shape.is_some();
        session.splash.state.visibility(content, visible);
    }
    match event {
        Event::WindowEvent { window_id, event, .. } => {
            if session.main.is_some() {
                main_operation(session, content, |main| main.window_event(window_id, &event))?;
            }
            if !session.closing
                && !session.splash.retiring
                && session.splash.window.as_ref().is_some_and(|window| window.id() == window_id)
            {
                let effect = input::dispatch(
                    content,
                    session.splash.window()?,
                    event,
                    &mut session.pointer,
                    &mut session.pressed,
                )?;
                if let Some(backward) = effect.focus {
                    session
                        .splash
                        .controls
                        .as_ref()
                        .ok_or_else(|| {
                            DesktopError::message("splash_focus", "original controls absent")
                        })?
                        .focus_next(backward)?;
                }
                if effect.close {
                    session.observe(content, DesktopStartupObservation::CloseRequested)?;
                }
                if effect.dirty {
                    session.splash.state.dirty();
                }
                if effect.expose {
                    session.splash.window()?.request_redraw();
                }
            }
        }
        Event::UserEvent(NativeAction::Close) if !session.closing => {
            session.observe(content, DesktopStartupObservation::CloseRequested)?;
        }
        Event::UserEvent(NativeAction::Activate(id))
            if !session.closing && !session.splash.retiring =>
        {
            if content.activate(id) {
                session.splash.state.dirty();
            }
        }
        Event::UserEvent(action @ NativeAction::MainControl(..)) if !session.closing => {
            if let Some(main) = &session.main
                && let Some(id) = action.main_control(main.identity())
            {
                main_operation(session, content, |main| main.activate(id))?;
            }
        }
        Event::UserEvent(NativeAction::MainBackendRetry(action)) if !session.closing => {
            let observation =
                if let Some(main) = &mut session.main { main.retry_backend(action)? } else { None };
            if let Some(observation) = observation {
                session.observe(content, observation)?;
            }
        }
        Event::UserEvent(NativeAction::FocusNext(backward))
            if !session.closing && !session.splash.retiring =>
        {
            session
                .splash
                .controls
                .as_ref()
                .ok_or_else(|| DesktopError::message("splash_focus", "original controls absent"))?
                .focus_next(backward)?;
        }
        Event::UserEvent(NativeAction::FocusChanged)
            if !session.closing && !session.splash.retiring =>
        {
            session.splash.window()?.request_redraw();
        }
        Event::UserEvent(NativeAction::Completions) => {
            if session.splash.state.poll(content, shape, session.drain, session.started.elapsed())
                && !session.splash.retiring
            {
                session.splash.window()?.request_redraw();
            }
        }
        Event::RedrawRequested(id)
            if session.splash.window.as_ref().is_some_and(|window| window.id() == id) =>
        {
            presentation::submit(session, content, shape, context.timings)?;
        }
        Event::MainEventsCleared => {
            if session.splash.state.poll(content, shape, session.drain, session.started.elapsed())
                && !session.splash.retiring
            {
                session.splash.window()?.request_redraw();
            }
            if session.splash.presentations.as_ref().is_some_and(|owner| owner.preparing_first()) {
                presentation::submit(session, content, shape, context.timings)?;
            }
            if session.closing {
                session.splash.state.poll_shutdown(content);
            } else if !session.splash.retiring {
                let now = Instant::now();
                let due = session.splash.state.visible
                    && content.animating()
                    && now >= session.next_frame;
                session.splash.state.prepare(content, shape, session.started.elapsed(), due);
                if due {
                    session.next_frame = advance_deadline(session.next_frame, now);
                }
            }
            progress_main(session, content, target, proxy.clone(), context.title)?;
            if session.splash.retiring && !session.splash.retired && session.splash.retire_step()? {
                session.splash_retirement_reported = true;
                session.observe(
                    content,
                    DesktopStartupObservation::SplashRetired(session.splash.launch),
                )?;
            }
            commands::drain(session, content, proxy)?;
        }
        _ => {}
    }
    presentation::reconcile(session, content, context.timings)?;
    activity(session, content)?;
    if let Some(observation) = session.activity.retirement(native_activity::RetirementFacts {
        main: session.main.as_ref().is_none_or(MainActor::retired),
        splash: session.splash.retired,
        observers: session.observers.retired(),
    }) {
        session.observe(content, observation)?;
    }
    if let Some(cause) = session.splash.state.failure.take() {
        return Err(cause);
    }
    Ok(())
}

/// Drain actual callback facts before any challenge can respond on this UI dispatch.
fn activity(
    session: &mut Session,
    content: &mut impl StartupWindowContent,
) -> Result<(), DesktopError> {
    session.observers.synchronize([
        if session.splash.retiring { None } else { session.splash.window.as_ref() },
        session.main.as_ref().and_then(MainActor::activity_window),
    ])?;
    for _ in 0..16 {
        let Some(fact) = session.observers.take()? else { break };
        if let Some(observation) = session.activity.observe(fact)? {
            session.observe(content, observation)?;
        }
    }
    Ok(())
}

fn progress_main(
    session: &mut Session,
    content: &mut impl StartupWindowContent,
    target: &EventLoopWindowTarget<NativeAction>,
    proxy: EventLoopProxy<NativeAction>,
    title: &str,
) -> Result<(), DesktopError> {
    let Some(main) = &mut session.main else {
        return Ok(());
    };
    match main.progress(target, proxy, title) {
        Ok(Some(observation)) => session.observe(content, observation),
        Ok(None) => Ok(()),
        Err(cause) => fail_main(session, content, cause),
    }
}

/// Close current native admission before transferring its original failure once.
fn fail_main(
    session: &mut Session,
    content: &mut impl StartupWindowContent,
    cause: DesktopError,
) -> Result<(), DesktopError> {
    let owner = main(session)?;
    let identity = owner.identity();
    let observation = owner.failure_observation();
    owner.close(identity)?;
    if session.main_failures_reported {
        session.retain(cause);
        Ok(())
    } else {
        session.main_failures_reported = true;
        cause.observe_main_failure(identity, observation);
        session.observe(
            content,
            DesktopStartupObservation::Failed {
                identity: Some(identity),
                failure: StartupFailure::new(StartupFailureCode::Presentation, Box::new(cause)),
            },
        )
    }
}

/// Native command failures retain the actor for the same checked retirement as progress failures.
fn main_operation(
    session: &mut Session,
    content: &mut impl StartupWindowContent,
    effect: impl FnOnce(&mut MainActor) -> Result<Option<DesktopStartupObservation>, DesktopError>,
) -> Result<(), DesktopError> {
    match effect(main(session)?) {
        Ok(Some(observation)) => session.observe(content, observation),
        Ok(None) => Ok(()),
        Err(cause) => fail_main(session, content, cause),
    }
}

fn main(session: &mut Session) -> Result<&mut MainActor, DesktopError> {
    session
        .main
        .as_mut()
        .ok_or_else(|| DesktopError::message("startup_main_command", "main owner missing"))
}

pub(super) fn wait(
    session: &Session,
    content: &impl StartupWindowContent,
    limit: Option<Instant>,
) -> ControlFlow {
    let deadline = wait_deadline(
        content.next_deadline(),
        limit,
        session.main.as_ref().and_then(MainActor::next_probe_deadline),
    );
    if session.closing
        || (session.splash.retiring && !session.splash.retired)
        || session.main.as_ref().is_some_and(MainActor::needs_progress)
    {
        let now = Instant::now();
        match (closing_wait(now), deadline.filter(|deadline| *deadline > now)) {
            (ControlFlow::WaitUntil(native), Some(deadline)) => {
                ControlFlow::WaitUntil(native.min(deadline))
            }
            (native, _) => native,
        }
    } else {
        presentation_wait(
            session.splash.state.visible,
            session.splash.retired,
            content.animating().then_some(session.next_frame),
            deadline,
        )
    }
}

/// Keep incomplete DOM retries independent of splash visibility and animation.
fn wait_deadline(
    application: Option<Instant>,
    limit: Option<Instant>,
    probe: Option<Instant>,
) -> Option<Instant> {
    [application, limit, probe].into_iter().flatten().min()
}

#[cfg(test)]
#[path = "startup_wait_tests.rs"]
mod wait_tests;
