// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Dispatch bounded application commands and native observations through existing owners.

use super::*;
use crate::{
    DesktopStartupCommand,
    cadence::{advance_deadline, closing_wait, presentation_wait},
    native_surface::{input, invalidate},
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
    if context.limit.is_some_and(|deadline| Instant::now() >= deadline) {
        session.close(content);
    }
    session.splash.bind_launch(content.launch_id());
    let shape = session.splash.shape()?;
    if !session.splash.retiring {
        let visible = session
            .splash
            .window
            .as_ref()
            .is_some_and(|window| window.is_visible() && !window.is_minimized())
            && shape.is_some();
        session.splash.state.visibility(content, visible);
    }
    match event {
        Event::WindowEvent { window_id, event, .. } => {
            if let Some(main) = &mut session.main
                && main.window_event(window_id, &event)?
            {
                session.observe(content, DesktopStartupObservation::CloseRequested)?;
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
                if effect.close {
                    session.observe(content, DesktopStartupObservation::CloseRequested)?;
                }
                if effect.dirty {
                    session.splash.state.dirty();
                }
                if effect.expose {
                    invalidate(session.splash.window()?, &mut session.redraw);
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
                && main.activate(id)?
            {
                session.observe(content, DesktopStartupObservation::CloseRequested)?;
            }
        }
        Event::UserEvent(NativeAction::Disclosure)
            if !session.closing && !session.splash.retiring =>
        {
            if content.key(crate::KeyAction::Tab) {
                session.splash.state.dirty();
            }
        }
        Event::UserEvent(NativeAction::FocusChanged)
            if !session.closing && !session.splash.retiring =>
        {
            invalidate(session.splash.window()?, &mut session.redraw);
        }
        Event::UserEvent(NativeAction::Completions) => {
            if session.splash.state.poll(content, shape, session.drain, session.started.elapsed())
                && !session.splash.retiring
            {
                invalidate(session.splash.window()?, &mut session.redraw);
            }
        }
        Event::RedrawRequested(id)
            if session.splash.window.as_ref().is_some_and(|window| window.id() == id) =>
        {
            session.redraw = false;
            if !session.splash.retiring
                && let Some(frame) = session.splash.state.exposable(shape)
            {
                let window = session.splash.window.as_ref().ok_or_else(|| {
                    DesktopError::message("present_startup_splash", "window missing")
                })?;
                let presenter = session.splash.presenter.as_mut().ok_or_else(|| {
                    DesktopError::message("present_startup_splash", "presenter missing")
                })?;
                let controls = session.splash.controls.as_mut().ok_or_else(|| {
                    DesktopError::message("present_startup_splash", "controls missing")
                })?;
                session
                    .splash
                    .presentations
                    .as_mut()
                    .ok_or_else(|| {
                        DesktopError::message(
                            "present_startup_splash",
                            "presentation owner missing",
                        )
                    })?
                    .submit(window, presenter, frame, context.timings, content, controls)?;
                if !session.splash.presented || session.splash.launch != content.launch_id() {
                    session.splash.presented = true;
                    session.splash.launch = content.launch_id();
                    session.observe(
                        content,
                        DesktopStartupObservation::SplashPresented(session.splash.launch),
                    )?;
                }
            }
        }
        Event::MainEventsCleared => {
            if session.splash.state.poll(content, shape, session.drain, session.started.elapsed())
                && !session.splash.retiring
            {
                invalidate(session.splash.window()?, &mut session.redraw);
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
            commands(session, content, proxy)?;
        }
        _ => {}
    }
    if let Some(cause) = session.splash.state.failure.take() {
        return Err(cause);
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
    owner.close(identity)?;
    if session.main_failures_reported {
        session.retain(cause);
        Ok(())
    } else {
        session.main_failures_reported = true;
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

fn commands(
    session: &mut Session,
    content: &mut impl StartupWindowContent,
    proxy: EventLoopProxy<NativeAction>,
) -> Result<(), DesktopError> {
    for _ in 0..32 {
        let Some(command) = content
            .take_command()
            .map_err(|cause| DesktopError::boxed("take_startup_command", cause))?
        else {
            return Ok(());
        };
        if session.closing
            && !matches!(
                command,
                DesktopStartupCommand::Close | DesktopStartupCommand::RetireMain(_)
            )
        {
            return Err(DesktopError::message("startup_command", "startup admission is closed"));
        }
        match command {
            DesktopStartupCommand::PrepareMain(request) => {
                if !session.splash.presented
                    || session.splash.retiring
                    || request.identity().launch != content.launch_id()
                    || session.splash.launch != request.identity().launch
                    || session.main.as_ref().is_some_and(|main| !main.retired())
                {
                    return Err(DesktopError::message(
                        "prepare_main",
                        "splash not submitted or prior main lifetime remains",
                    ));
                }
                if let Some(previous) = session.main.take() {
                    session.navigations += previous.navigations();
                    session.main_material = previous.material();
                }
                session.main_failures_reported = false;
                session.main = Some(MainActor::new(*request, proxy.clone())?);
            }
            DesktopStartupCommand::AuthorizeEnvironment(identity) => {
                main_operation(session, content, |owner| {
                    owner.authorize_environment(identity).map(|()| None)
                })?
            }
            DesktopStartupCommand::AuthorizeBrowser(identity) => {
                main_operation(session, content, |owner| {
                    owner.authorize_browser(identity).map(|()| None)
                })?
            }
            DesktopStartupCommand::ProbeFrontend(identity) => {
                main_operation(session, content, |owner| owner.probe(identity).map(|()| None))?;
            }
            DesktopStartupCommand::RetireMain(identity) => main(session)?.close(identity)?,
            DesktopStartupCommand::RetireSplash(launch) => {
                if launch != session.splash.launch || !session.splash.presented {
                    return Err(DesktopError::message(
                        "retire_splash",
                        "splash identity stale or not submitted",
                    ));
                }
                session.splash.begin_retirement(content)?;
            }
            DesktopStartupCommand::RevealMain(identity) => {
                if !session.splash.retired || !session.splash_retirement_reported {
                    return Err(DesktopError::message(
                        "reveal_main",
                        "actual splash retirement is missing",
                    ));
                }
                main_operation(session, content, |owner| owner.reveal(identity).map(Some))?;
            }
            DesktopStartupCommand::Close => session.close(content),
        }
    }
    proxy
        .send_event(NativeAction::Main)
        .map_err(|cause| DesktopError::new("rewake_startup_commands", cause))
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
    let application = content.next_deadline();
    let deadline = match (application, limit) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    };
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
