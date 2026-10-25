// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Pump native canvas input, borrowed presentation and actual application teardown on one UI STA.
use super::*;
use crate::{
    SelectionKind,
    completion_wake::{ProxyWake, WakeLifetime},
};
use ::windows::Win32::Foundation::HWND;
use std::time::Instant;
use tao::{
    dpi::PhysicalSize,
    event::{ElementState, Event, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    keyboard::Key,
    platform::{run_return::EventLoopExtRunReturn, windows::WindowExtWindows},
    window::WindowBuilder,
};

/// Run a standalone native canvas without starting a browser, ComfyUI or inference connection.
/// Call on the OS main thread. CPU image work remains behind the application's lifetime port.
/// The original window/event loop stay alive until application workers and native peers retire.
/// # Errors
/// Preserves construction, paint, application and teardown failures; cancellation never implies exit.
pub fn run(options: WindowOptions, lifetime: &mut impl Lifetime) -> Result<(), Box<dyn Error>> {
    let mut apartment = crate::windows_ui_apartment::UiApartment::enter()?;
    let mut builder = EventLoopBuilder::<()>::with_user_event();
    #[cfg(any(test, feature = "boundary-fixtures"))]
    {
        use tao::platform::windows::EventLoopBuilderExtWindows;
        builder.with_any_thread(true);
    }
    let mut events = builder.build();
    let window = match WindowBuilder::new()
        .with_title(&options.title)
        .with_inner_size(PhysicalSize::new(options.extent.width(), options.extent.height()))
        .with_min_inner_size(PhysicalSize::new(64, 64))
        .with_max_inner_size(PhysicalSize::new(3840, 2160))
        .with_visible(options.visible)
        .build(&events)
    {
        Ok(window) => Arc::new(window),
        Err(cause) => {
            drop(events);
            // SAFETY: Construction failed before a window, callback or COM receiver escaped.
            let retired = unsafe { apartment.release_after_teardown() };
            return match retired {
                Ok(()) => Err(cause.into()),
                Err(retirement) => Err(Box::new(NativeFailures {
                    primary: Box::new(cause),
                    retirement: Box::new(retirement),
                })),
            };
        }
    };
    if !options.visible {
        // Windows constrains creation bounds to the monitor work area. Hidden qualification
        // explicitly restores its requested physical client extent before graphics attachment.
        window.set_inner_size(PhysicalSize::new(options.extent.width(), options.extent.height()));
    }
    let wake = WakeLifetime::new();
    let mut failure =
        lifetime.start(Arc::new(ProxyWake::new(events.create_proxy(), wake.clone(), ()))).err();
    if failure.is_none() {
        failure = lifetime.attach_window(WindowTarget(window.clone())).err();
    }
    let mut closing = failure.is_some();
    let mut close_started = false;
    let mut painter = super::paint::Painter::default();
    let mut displayed = None;
    let mut extent = None;
    let mut title = options.title;
    let mut control_down = false;
    let mut pan_buttons = 0_u8;
    let started = Instant::now();
    let mut stopped = false;
    while !stopped {
        let status = events.run_return(|event, _, control| {
        *control = ControlFlow::Wait;
        let result = (|| -> Result<(), Box<dyn Error>> {
            match event {
                Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => closing = true,
                Event::WindowEvent { event: WindowEvent::ModifiersChanged(modifiers), .. } => control_down = modifiers.control_key(),
                Event::WindowEvent { event: WindowEvent::CursorMoved { position, .. }, .. } if !closing => lifetime.input(Input::Pointer([position.x, position.y]))?,
                Event::WindowEvent { event: WindowEvent::MouseInput { state, button: button @ (MouseButton::Left | MouseButton::Middle), .. }, .. } if !closing => {
                    let bit = if button == MouseButton::Left { 1 } else { 2 };
                    if state == ElementState::Pressed { pan_buttons |= bit; } else { pan_buttons &= !bit; }
                    lifetime.input(Input::Pan(pan_buttons != 0))?;
                }
                Event::WindowEvent { event: WindowEvent::Focused(false), .. } if !closing => {
                    control_down = false; pan_buttons = 0; lifetime.input(Input::Unfocused)?;
                }
                Event::WindowEvent { event: WindowEvent::MouseWheel { delta, .. }, .. } if !closing => {
                    let delta = match delta {
                        MouseScrollDelta::LineDelta(x, y) => WheelDelta::Lines([f64::from(x), f64::from(y)]),
                        MouseScrollDelta::PixelDelta(position) => WheelDelta::Pixels([position.x, position.y]),
                        other => return Err(DesktopError::message("canvas_wheel", format!("Unsupported native wheel observation: {other:?}")).into()),
                    };
                    lifetime.input(Input::Wheel(delta))?;
                }
                Event::WindowEvent { event: WindowEvent::KeyboardInput { event, .. }, .. } if !closing && event.state == ElementState::Pressed && !event.repeat => {
                    let command = match event.logical_key {
                        Key::Character(value) if control_down && value.eq_ignore_ascii_case("o") => Some(Input::Open),
                        Key::Character(value) if value.eq_ignore_ascii_case("f") => Some(Input::Fit),
                        Key::Character("1") => Some(Input::ActualSize),
                        _ => None,
                    };
                    if let Some(command) = command { lifetime.input(command)?; }
                }
                Event::RedrawRequested(_) if !closing => {
                    if let Some(frame) = lifetime.frame() {
                        painter.present(HWND(window.hwnd() as *mut _), &frame, true)?;
                        displayed = Some(frame.target);
                    }
                    lifetime.expose();
                }
                Event::MainEventsCleared => {
                    closing |= lifetime.is_closing();
                    if closing && !close_started {
                        lifetime.close()?;
                        close_started = true;
                    }
                    if !closing {
                        let size = window.inner_size();
                        if size.width > 0 && size.height > 0 {
                            let next = Extent::new(size.width, size.height)?;
                            if extent != Some(next) { lifetime.resize(next)?; extent = Some(next); }
                        }
                    }
                    let advanced = lifetime.poll(started.elapsed());
                    if closing {
                        let settled = lifetime.settled();
                        if matches!(settled, Ok(true)) { stopped = true; *control = ControlFlow::Exit; }
                        combine(advanced, settled.map(|_| ()))?;
                    } else {
                        advanced?;
                        if lifetime.is_closing() {
                            closing = true;
                            return Ok(());
                        }
                        if title != lifetime.title() {
                            title = lifetime.title().to_owned();
                            window.set_title(&title);
                        }
                        if let Some(frame) = lifetime.frame() && displayed != Some(frame.target) {
                            painter.present(HWND(window.hwnd() as *mut _), &frame, false)?;
                            displayed = Some(frame.target);
                        }
                        if let Some(frame) = lifetime.take_gpu_presentation() {
                            let target = frame.target;
                            frame.frame.present()?;
                            displayed = Some(target);
                            lifetime.did_present_gpu(target)?;
                            lifetime.poll(started.elapsed())?;
                        }
                        if lifetime.take_picker_request() {
                            lifetime.selected(crate::file_dialog::choose(&window, SelectionKind::File))?;
                            lifetime.poll(started.elapsed())?;
                        }
                    }
                }
                _ => {}
            }
            Ok(())
        })();
        if let Err(cause) = result {
            tracing::error!(operation="native_canvas_lifetime", cause=%cause, "Native canvas entered cleanup");
            if failure.is_none() { failure = Some(cause); }
            closing = true;
        }
        if closing && *control != ControlFlow::Exit {
            *control = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(16));
        } else if !closing && let Some(deadline) = lifetime.next_deadline() {
            *control = ControlFlow::WaitUntil(deadline);
        }
        });
        if !stopped {
            if failure.is_none() {
                failure = Some(
                    DesktopError::message(
                        "canvas_native_return",
                        format!(
                            "Native canvas loop returned before application settlement: {status}"
                        ),
                    )
                    .into(),
                );
            }
            closing = true;
        }
    }
    wake.disconnect();
    let original = window.id();
    drop(window);
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut destroyed = false;
    events.run_return(|event, _, control| {
        *control = ControlFlow::WaitUntil(deadline);
        match event {
            Event::WindowEvent { window_id, event: WindowEvent::Destroyed, .. }
                if window_id == original =>
            {
                destroyed = true;
                *control = ControlFlow::Exit;
            }
            Event::MainEventsCleared if Instant::now() >= deadline => *control = ControlFlow::Exit,
            _ => {}
        }
    });
    if !destroyed {
        return Err(DesktopError::message(
            "canvas_window_retirement",
            "Native canvas destruction was not observed",
        )
        .into());
    }
    drop(events);
    tracing::info!(
        operation = "native_canvas_retirement",
        "Native canvas window destruction observed; event loop retired"
    );
    // SAFETY: All native window events and modal COM receivers retired on this same UI thread.
    let retired = unsafe { apartment.release_after_teardown() }.map_err(Box::<dyn Error>::from);
    let finished = lifetime.finish();
    combine(combine(failure.map_or(Ok(()), Err), retired), finished)
}

/// Retain primary and later native/application failures independently through finalization.
fn combine(
    first: Result<(), Box<dyn Error>>,
    second: Result<(), Box<dyn Error>>,
) -> Result<(), Box<dyn Error>> {
    match (first, second) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(cause), Ok(())) | (Ok(()), Err(cause)) => Err(cause),
        (Err(primary), Err(retirement)) => Err(Box::new(NativeFailures { primary, retirement })),
    }
}
#[derive(Debug)]
struct NativeFailures {
    primary: Box<dyn Error>,
    retirement: Box<dyn Error>,
}
impl std::fmt::Display for NativeFailures {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}; native/application retirement: {}", self.primary, self.retirement)
    }
}
impl Error for NativeFailures {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.primary.as_ref())
    }
}
