// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Pump native canvas input, borrowed presentation and actual application teardown on one UI STA.
mod cursor;
mod display;
mod input;
use super::*;
use crate::{
    SelectionKind,
    completion_wake::{ProxyWake, WakeLifetime},
};
use ::windows::Win32::Foundation::HWND;
use input::{NativeEvent, NativeInput};
use std::time::Instant;
use sugarsubstitute_canvas::{ScrollPhase, ToolButton, ToolInput};
use tao::{
    dpi::PhysicalSize,
    event::{ElementState, Event, MouseButton, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    keyboard::Key,
    platform::{
        run_return::EventLoopExtRunReturn,
        windows::{MonitorHandleExtWindows, WindowExtWindows},
    },
    window::WindowBuilder,
};

/// Run a standalone native canvas without starting a browser, ComfyUI or inference connection.
/// Call on the OS main thread. CPU image work remains behind the application's lifetime port.
/// The original window/event loop stay alive until application workers and native peers retire.
/// # Errors
/// Preserves construction, paint, application and teardown failures; cancellation never implies exit.
pub fn run(options: WindowOptions, lifetime: &mut impl Lifetime) -> Result<(), Box<dyn Error>> {
    run_with_picker(options, lifetime, |window| {
        crate::file_dialog::choose(window, SelectionKind::File)
    })
}

fn run_with_picker(
    options: WindowOptions,
    lifetime: &mut impl Lifetime,
    mut choose: impl FnMut(&tao::window::Window) -> Result<Option<PathBuf>, Box<dyn Error>>,
) -> Result<(), Box<dyn Error>> {
    let mut apartment = crate::windows_ui_apartment::UiApartment::enter()?;
    let mut builder = EventLoopBuilder::<NativeEvent>::with_user_event();
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
    let hwnd = HWND(window.hwnd() as *mut _);
    let mut failure = None;
    let input_proxy = events.create_proxy();
    let native = match NativeInput::new(hwnd, move |event| {
        input_proxy
            .send_event(event)
            .map_err(|cause| DesktopError::new("canvas_native_input", cause))
    }) {
        Ok(input) => Some(input),
        Err(cause) => {
            failure = Some(cause);
            None
        }
    };
    if failure.is_none() {
        failure = lifetime
            .start(Arc::new(ProxyWake::new(events.create_proxy(), wake.clone(), NativeEvent::Wake)))
            .err();
    }
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
    let mut display_id = None;
    let mut display_dirty = true;
    let started = Instant::now();
    let mut stopped = false;
    while !stopped {
        let status = events.run_return(|event, _, control| {
        *control = ControlFlow::Wait;
        let result = (|| -> Result<(), Box<dyn Error>> {
            match event {
                Event::UserEvent(NativeEvent::Wheel { point, steps }) if !closing => {
                    lifetime.input(Input::Tool(ToolInput::Pointer(point)))?;
                    lifetime.input(Input::Tool(ToolInput::Wheel { steps, phase: ScrollPhase::Unphased }))?;
                }
                Event::UserEvent(NativeEvent::DoubleClick(point)) if !closing => lifetime.input(Input::Tool(ToolInput::DoubleClick(point)))?,
                Event::UserEvent(NativeEvent::CaptureLost) if !closing => lifetime.input(Input::Unfocused)?,
                Event::UserEvent(NativeEvent::DisplayChanged) if !closing => display_dirty = true,
                Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => closing = true,
                Event::WindowEvent { event: WindowEvent::ModifiersChanged(modifiers), .. } => control_down = modifiers.control_key(),
                Event::WindowEvent { event: WindowEvent::CursorMoved { position, .. }, .. } if !closing => lifetime.input(Input::Tool(ToolInput::Pointer([position.x, position.y])))?,
                Event::WindowEvent { event: WindowEvent::MouseInput { state, button: button @ (MouseButton::Left | MouseButton::Middle), .. }, .. } if !closing => {
                    let button = if button == MouseButton::Left { ToolButton::Primary } else { ToolButton::Middle };
                    lifetime.input(Input::Tool(ToolInput::Button { button, pressed: state == ElementState::Pressed }))?;
                }
                Event::WindowEvent { event: WindowEvent::Focused(false), .. } if !closing => {
                    control_down = false; lifetime.input(Input::Unfocused)?;
                }
                Event::WindowEvent { event: WindowEvent::KeyboardInput { event, .. }, .. } if !closing && !event.repeat => {
                    if event.logical_key == Key::Space {
                        lifetime.input(Input::TemporaryNavigation(event.state == ElementState::Pressed))?;
                    } else if event.state == ElementState::Pressed {
                        let command = match event.logical_key {
                            Key::Character(value) if control_down && value.eq_ignore_ascii_case("o") => Some(Input::Open),
                            Key::Character(value) if value.eq_ignore_ascii_case("f") => Some(Input::Tool(ToolInput::Fit)),
                            Key::Character("1") => Some(Input::Tool(ToolInput::OneToOne)),
                            _ => None,
                        };
                        if let Some(command) = command { lifetime.input(command)?; }
                    }
                }
                Event::RedrawRequested(_) if !closing => {
                    if let Some(frame) = lifetime.frame() {
                        let target = frame.target;
                        let pending = frame.pending;
                        painter.present(HWND(window.hwnd() as *mut _), &frame, true)?;
                        if record_cpu_upload(&mut displayed, target, pending, |target| {
                            lifetime.did_present_cpu(target)
                        })? {
                            lifetime.poll(started.elapsed())?;
                        }
                    }
                    lifetime.expose();
                }
                Event::MainEventsCleared => {
                    if let Some(native) = &native { native.check()?; }
                    closing |= lifetime.is_closing();
                    if closing && !close_started {
                        lifetime.close()?;
                        close_started = true;
                    }
                    if !closing {
                        let monitor = window.current_monitor();
                        let identity = monitor.as_ref().map(MonitorHandleExtWindows::hmonitor);
                        if display_dirty || display_id != identity {
                            let rate = match monitor.as_ref().map(display::refresh).transpose() {
                                Ok(rate) => rate.flatten(),
                                Err(cause) => {
                                    tracing::warn!(operation="canvas_display_fallback", cause=%cause,
                                        "Active display timing unavailable; using 60 Hz");
                                    None
                                }
                            };
                            lifetime.input(Input::DisplayRefresh(rate))?;
                            tracing::info!(operation="canvas_display_timing", hertz=rate.unwrap_or(60.0),
                                "Native canvas display timing updated");
                            display_id = identity;
                            display_dirty = false;
                        }
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
                        if let Some(native) = &native {
                            let feedback = lifetime.feedback();
                            native.cursor(feedback.cursor)?;
                            if !native.capture(feedback.capture)? { lifetime.input(Input::Unfocused)?; }
                        }
                        if title != lifetime.title() {
                            title = lifetime.title().to_owned();
                            window.set_title(&title);
                        }
                        if let Some(frame) = lifetime.frame() && displayed != Some(frame.target) {
                            let target = frame.target;
                            let pending = frame.pending;
                            painter.present(HWND(window.hwnd() as *mut _), &frame, false)?;
                            if record_cpu_upload(&mut displayed, target, pending, |target| {
                                lifetime.did_present_cpu(target)
                            })? {
                                lifetime.poll(started.elapsed())?;
                            }
                        }
                        if let Some(frame) = lifetime.take_gpu_presentation() {
                            let target = frame.target;
                            let presentation = frame.frame.present()?;
                            displayed = Some(target);
                            lifetime.did_present_gpu(target)?;
                            if presentation.needs_follow_up {
                                lifetime.expose();
                            }
                            lifetime.poll(started.elapsed())?;
                        }
                        if lifetime.take_picker_request() {
                            lifetime.input(Input::Unfocused)?;
                            if let Some(native) = &native { native.capture(false)?; }
                            lifetime.selected(choose(&window))?;
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
    if !destroyed || native.as_ref().is_some_and(|native| !native.retired()) {
        return Err(DesktopError::message(
            "canvas_window_retirement",
            "Native canvas destruction was not observed",
        )
        .into());
    }
    drop(events);
    let input_retired = native.as_ref().map_or(Ok(()), NativeInput::retire);
    tracing::info!(
        operation = "native_canvas_retirement",
        "Native canvas window destruction observed; event loop retired"
    );
    // SAFETY: All native window events and modal COM receivers retired on this same UI thread.
    let retired = unsafe { apartment.release_after_teardown() }.map_err(Box::<dyn Error>::from);
    let finished = lifetime.finish();
    combine(combine(combine(failure.map_or(Ok(()), Err), input_retired), retired), finished)
}

/// Acknowledge a pending synchronous CPU upload before making its native identity current.
fn record_cpu_upload<E>(
    displayed: &mut Option<FrameStamp>,
    target: FrameStamp,
    pending: bool,
    acknowledge: impl FnOnce(FrameStamp) -> Result<(), E>,
) -> Result<bool, E> {
    if pending {
        acknowledge(target)?;
    }
    *displayed = Some(target);
    Ok(pending)
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

#[cfg(test)]
#[path = "windows/tests.rs"]
mod tests;
