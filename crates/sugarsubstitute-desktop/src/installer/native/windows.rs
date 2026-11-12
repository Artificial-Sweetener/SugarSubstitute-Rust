// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Present prepared installer pages using the original native window and shared form peers.
use super::*;
use crate::{
    completion_wake::{ProxyWake, WakeLifetime},
    native_action::{FormScope, NativeAction},
    windows_controls::Controls,
};
use std::time::Instant;
use tao::{
    event::{Event as NativeEvent, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    platform::run_return::EventLoopExtRunReturn,
};
#[path = "form_window.rs"]
mod form_window;
#[path = "projection.rs"]
mod projection;
pub(crate) use form_window::{FormInput, FormOwner, FormTurn, FormWindow};
use projection::Projection;
#[cfg(test)]
use {form_window::paint_theme, tao::window::Theme};

/// Own a single event loop from hidden construction through checked native and application teardown.
pub(super) fn run(
    options: super::Window,
    lifetime: &mut impl Lifetime,
) -> Result<(), Box<dyn Error>> {
    let mut apartment = crate::windows_ui_apartment::UiApartment::enter()?;
    let mut builder = EventLoopBuilder::<NativeAction>::with_user_event();
    #[cfg(any(test, feature = "boundary-fixtures"))]
    {
        use tao::platform::windows::EventLoopBuilderExtWindows;
        builder.with_any_thread(true);
    }
    let picker = crate::file_dialog::Picker::new(&mut builder);
    let mut events = builder.build();
    let proxy = events.create_proxy();
    let wake_lifetime = WakeLifetime::new();
    let mut accent =
        crate::windows_system_accent::SystemAccent::observe(proxy.clone(), wake_lifetime.clone());
    let wake =
        Arc::new(ProxyWake::new(proxy.clone(), wake_lifetime.clone(), NativeAction::Completions));
    let scope = FormScope(0);
    let mut form = FormWindow::open(
        &events,
        proxy,
        scope,
        picker,
        None,
        options,
        accent.take_change().flatten(),
        lifetime,
        wake,
    )?;
    let started = Instant::now();
    events.run_return(|event, _, control| {
        *control = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(20));
        match event {
            NativeEvent::WindowEvent { window_id, event, .. } if window_id == form.window_id() => {
                form.input(FormInput::Window(&event), lifetime);
            }
            NativeEvent::UserEvent(NativeAction::Form(action_scope, action))
                if action_scope == scope =>
            {
                form.input(FormInput::Action(action), lifetime);
            }
            NativeEvent::UserEvent(NativeAction::SystemAccent) => {
                if let Some(current) = accent.take_change() {
                    form.input(FormInput::Accent(current), lifetime);
                }
            }
            NativeEvent::RedrawRequested(id) if id == form.window_id() => {
                form.input(FormInput::Redraw, lifetime);
            }
            NativeEvent::MainEventsCleared => {
                if form.turn(lifetime) == FormTurn::Released {
                    *control = ControlFlow::Exit;
                }
            }
            _ => {}
        }
    });
    accent.retire();
    let (original, picker, mut failure) = form.retire();
    let retain = |cause: Box<dyn Error>, failure: &mut Option<Box<dyn Error>>| {
        if failure.is_none() {
            *failure = Some(cause);
        } else {
            tracing::error!(operation="native_installer_settlement", cause=%cause, "Additional failure after window retirement");
        }
    };
    if let Err(cause) = picker.retire() {
        retain(Box::new(cause), &mut failure);
    }
    drop(picker);
    // Tao posts its destruction request from Drop. Keep pumping the same event loop until the
    // original OS window reports destruction, rather than mistaking an Rc drop for retirement.
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut destroyed = false;
    events.run_return(|event, _, control| {
        *control = ControlFlow::WaitUntil(deadline);
        match event {
            NativeEvent::WindowEvent { window_id, event: WindowEvent::Destroyed, .. }
                if window_id == original =>
            {
                destroyed = true;
                *control = ControlFlow::Exit;
            }
            NativeEvent::MainEventsCleared if Instant::now() >= deadline => {
                *control = ControlFlow::Exit
            }
            _ => {}
        }
    });
    if !destroyed {
        return Err("Native installer window destruction was not observed".into());
    }
    if let Err(cause) = lifetime.window_retired() {
        retain(cause, &mut failure);
    }
    settle_after_retirement(&mut events, lifetime, started, &mut |cause| {
        retain(cause, &mut failure)
    });
    wake_lifetime.disconnect();
    drop(events);
    let finished = lifetime.finish();
    // SAFETY: Checked peers, render buffers, their event-loop window and the lifetime's final
    // owners all retired on this thread.
    let released = unsafe { apartment.release_after_teardown() };
    match (failure, finished, released) {
        (Some(cause), Err(retirement), _) => {
            Err(format!("{cause}; native installer finalization: {retirement}").into())
        }
        (Some(cause), _, _) => Err(cause),
        (None, Err(cause), _) => Err(cause),
        (None, Ok(()), released) => released.map_err(Into::into),
    }
}

/// Keep the event loop and completion wake live after the window has retired until the
/// lifetime has settled. An error ends settlement; `finish` retains what has not retired.
fn settle_after_retirement(
    events: &mut tao::event_loop::EventLoop<NativeAction>,
    lifetime: &mut impl Lifetime,
    started: Instant,
    retain: &mut impl FnMut(Box<dyn Error>),
) {
    match lifetime.settled() {
        Ok(true) => return,
        Ok(false) => {}
        Err(cause) => return retain(cause),
    }
    events.run_return(|event, _, control| {
        *control = crate::cadence::closing_wait(Instant::now());
        if let NativeEvent::MainEventsCleared = event {
            match lifetime.advance(started.elapsed()).and_then(|()| lifetime.settled()) {
                Ok(true) => *control = ControlFlow::Exit,
                Ok(false) => {}
                Err(cause) => {
                    retain(cause);
                    *control = ControlFlow::Exit;
                }
            }
        }
    });
}

/// Copy `transfer` as `window`, entering the clipboard only while its counter deadline holds.
fn copy_transfer(
    window: ::windows::Win32::Foundation::HWND,
    transfer: &ClipboardTransfer,
) -> Result<(), CopyFailure> {
    let admitted = crate::windows_clipboard::copy_text_if(window, &transfer.text, || {
        transfer.before.map_or(Ok(true), counter_before)
    });
    match admitted {
        Ok(true) => Ok(()),
        Ok(false) => Err(CopyFailure::Expired),
        Err(cause) => Err(CopyFailure::Transfer(cause.to_string())),
    }
}

/// Whether this thread's performance counter is still before `deadline`, read on the same
/// thread and clock source that the deadline was issued against. Nothing is renewed.
fn counter_before(
    deadline: sugarsubstitute_contracts::execution::CounterDeadline,
) -> Result<bool, crate::DesktopError> {
    use crate::timing::TimingClock;
    let mut clock = crate::windows_timing::WindowsClock::new()?;
    let stamp = clock.read()?;
    if stamp.thread != clock.thread() || stamp.source != crate::timing::QPC_SOURCE {
        return Err(crate::DesktopError::message(
            "report_copy_clock_identity",
            "performance counter identity changed",
        ));
    }
    Ok(stamp.ticks > 0
        && clock.frequency() == deadline.frequency()
        && stamp.ticks < deadline.ticks())
}

/// Give the window `size` physical client pixels. Tao enlarges a request for an undecorated
/// window with a shadow by its hidden frame, and not on every call, so each observed difference
/// is fed back into the next request. A window that still differs keeps its real size; the
/// lifetime is told that size and prepares for it.
fn apply_client_size(window: &tao::window::Window, size: [u32; 2]) {
    let mut request = size;
    for _ in 0..3 {
        window.set_inner_size(tao::dpi::PhysicalSize::new(request[0], request[1]));
        let actual = window.inner_size();
        if [actual.width, actual.height] == size {
            return;
        }
        let adjust = |request: u32, wanted: u32, got: u32| {
            (i64::from(request) + i64::from(wanted) - i64::from(got)).clamp(1, 4096) as u32
        };
        request =
            [adjust(request[0], size[0], actual.width), adjust(request[1], size[1], actual.height)];
    }
}

/// Initial document population and picker replacements also advance native edit revisions.
/// Return only mismatches so acknowledgement does not create an endless repaint loop.
fn next_observations(
    owner: &Controls,
    page: &Projection,
) -> Result<Vec<(ControlId, Input)>, Box<dyn Error>> {
    let mut result = Vec::new();
    for spec in &page.specifications {
        if matches!(spec.role, sugarsubstitute_ui::form::Role::TextField(_))
            && let Some(input @ Input::Text { revision, .. }) = owner.observe(spec.control.id)?
            && revision != spec.text_revision
        {
            result.push((ControlId::new(spec.control.id)?, input));
        }
    }
    Ok(result)
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
