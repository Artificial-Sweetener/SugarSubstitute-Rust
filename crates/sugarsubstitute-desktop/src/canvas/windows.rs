// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Pump native canvas input, borrowed presentation and actual application teardown on one UI STA.
mod activity;
pub(crate) mod cursor;
mod display;
mod input;
mod retirement;
use super::feedback;
use super::latency::Timing;
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
    event::{DeviceEvent, ElementState, Event, MouseButton, WindowEvent},
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
    run_with_picker(options, lifetime, crate::file_dialog::Picker::new)
}

/// Compose the native loop with one shared modal dispatcher installed before event-loop creation.
fn run_with_picker(
    options: WindowOptions,
    lifetime: &mut impl Lifetime,
    make_picker: impl FnOnce(&mut EventLoopBuilder<NativeEvent>) -> crate::file_dialog::Picker,
) -> Result<(), Box<dyn Error>> {
    let mut apartment = crate::windows_ui_apartment::UiApartment::enter()?;
    let mut builder = EventLoopBuilder::<NativeEvent>::with_user_event();
    #[cfg(any(test, feature = "boundary-fixtures"))]
    {
        use tao::platform::windows::EventLoopBuilderExtWindows;
        builder.with_any_thread(true);
    }
    let picker = make_picker(&mut builder);
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
    let picker_proxy = events.create_proxy();
    let mut failure = picker
        .bind(hwnd, move || {
            picker_proxy
                .send_event(NativeEvent::Wake)
                .map_err(|cause| DesktopError::new("canvas_picker_wake", cause))
        })
        .err()
        .map(Box::<dyn Error>::from);
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
        let target = WindowTarget(window.clone());
        failure = lifetime.attach_window(target).err();
    }
    if failure.is_none() {
        failure = feedback::synchronize(lifetime, native.as_ref()).err();
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
    let mut drawable = true;
    let started = Instant::now();
    let mut timing = Timing::default();
    let mut stopped = false;
    while !stopped {
        let status = events.run_return(|event, _, control| {
        let turn_started = Instant::now();
        let raw_motion = matches!(&event, Event::DeviceEvent { event: DeviceEvent::MouseMotion { delta, .. }, .. } if activity::pointer_motion(*delta, options.visible, window.is_focused()));
        let input_turn = raw_motion || matches!(&event, Event::UserEvent(NativeEvent::Wheel { .. } | NativeEvent::DoubleClick(_)) | Event::WindowEvent { event: WindowEvent::CursorMoved { .. } | WindowEvent::MouseInput { .. } | WindowEvent::KeyboardInput { .. }, .. });
        *control = ControlFlow::Wait;
        let result = (|| -> Result<(), Box<dyn Error>> {
            match event {
                Event::UserEvent(NativeEvent::Wheel { point, steps }) if !closing => {
                    feedback::dispatch_input(lifetime, native.as_ref(), Input::Tool(ToolInput::Pointer(point)))?;
                    feedback::dispatch_input(lifetime, native.as_ref(), Input::Tool(ToolInput::Wheel { steps, phase: ScrollPhase::Unphased }))?;
                }
                Event::UserEvent(NativeEvent::DoubleClick(point)) if !closing => feedback::dispatch_input(lifetime, native.as_ref(), Input::Tool(ToolInput::DoubleClick(point)))?,
                Event::UserEvent(NativeEvent::CaptureLost) if !closing => feedback::dispatch_input(lifetime, native.as_ref(), Input::Unfocused)?,
                Event::UserEvent(NativeEvent::DisplayChanged) if !closing => display_dirty = true,
                Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => closing = true,
                Event::WindowEvent { event: WindowEvent::ModifiersChanged(modifiers), .. } => control_down = modifiers.control_key(),
                Event::WindowEvent { event: WindowEvent::CursorMoved { position, .. }, .. } if !closing => {
                    feedback::dispatch_input(lifetime, native.as_ref(), Input::Tool(ToolInput::Pointer([position.x, position.y])))?;
                },
                Event::WindowEvent { event: WindowEvent::MouseInput { state, button: button @ (MouseButton::Left | MouseButton::Middle), .. }, .. } if !closing => {
                    let button = if button == MouseButton::Left { ToolButton::Primary } else { ToolButton::Middle };
                    feedback::dispatch_input(lifetime, native.as_ref(), Input::Tool(ToolInput::Button { button, pressed: state == ElementState::Pressed }))?;
                }
                Event::WindowEvent { event: WindowEvent::Focused(false), .. } if !closing => {
                    control_down = false; feedback::dispatch_input(lifetime, native.as_ref(), Input::Unfocused)?;
                }
                Event::WindowEvent { event: WindowEvent::KeyboardInput { event, .. }, .. } if !closing && !event.repeat => {
                    if event.logical_key == Key::Space {
                        feedback::dispatch_input(lifetime, native.as_ref(), Input::TemporaryNavigation(event.state == ElementState::Pressed))?;
                    } else if event.state == ElementState::Pressed {
                        let command = match event.logical_key {
                            Key::Character(value) if control_down && value.eq_ignore_ascii_case("o") => Some(Input::Open),
                            Key::Character(value) if value.eq_ignore_ascii_case("f") => Some(Input::Tool(ToolInput::Fit)),
                            Key::Character("1") => Some(Input::Tool(ToolInput::OneToOne)),
                            _ => None,
                        };
                        if let Some(command) = command { feedback::dispatch_input(lifetime, native.as_ref(), command)?; }
                    }
                }
                Event::RedrawRequested(_) if !closing => {
                    timing.exposure();
                    if let Some(frame) = lifetime.frame() {
                        let target = frame.target;
                        let pending = frame.pending;
                        painter.present(HWND(window.hwnd() as *mut _), &frame, true)?;
                        if record_cpu_upload(&mut displayed, target, pending, |target| {
                            lifetime.did_present_cpu(target)
                        })? {
                            timing.poll(|| feedback::poll(lifetime, native.as_ref(), started.elapsed()))?;
                        }
                    }
                    lifetime.expose();
                }
                Event::MainEventsCleared => {
                    if let Some(native) = &native { native.check()?; }
                    picker.check()?;
                    closing |= lifetime.is_closing();
                    if closing { picker.cancel()?; }
                    if let Some(result) = picker.take_result() {
                        if closing { result?; } else { feedback::selected(lifetime, native.as_ref(), result)?; }
                    }
                    if closing && !close_started {
                        feedback::close(lifetime, native.as_ref())?;
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
                            feedback::dispatch_input(lifetime, native.as_ref(), Input::DisplayRefresh(rate))?;
                            tracing::info!(operation="canvas_display_timing", hertz=rate.unwrap_or(60.0),
                                "Native canvas display timing updated");
                            display_id = identity;
                            display_dirty = false;
                        }
                        let available = !window.is_minimized();
                        if drawable != available {
                            drawable = available;
                            feedback::dispatch_input(lifetime, native.as_ref(), Input::Drawable(available))?;
                        }
                        let size = window.inner_size();
                        if size.width > 0 && size.height > 0 {
                            let next = Extent::new(size.width, size.height)?;
                            if extent != Some(next) {
                                feedback::resize(lifetime, native.as_ref(), next)?;
                                extent = Some(next);
                            }
                        }
                    }
                    let advanced = timing.poll(|| feedback::poll(lifetime, native.as_ref(), started.elapsed()));
                    if closing {
                        let settled = lifetime.settled();
                        if matches!(settled, Ok(true)) && !picker.busy() { stopped = true; *control = ControlFlow::Exit; }
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
                            let target = frame.target;
                            let pending = frame.pending;
                            painter.present(HWND(window.hwnd() as *mut _), &frame, false)?;
                            if record_cpu_upload(&mut displayed, target, pending, |target| {
                                lifetime.did_present_cpu(target)
                            })? {
                                timing.poll(|| feedback::poll(lifetime, native.as_ref(), started.elapsed()))?;
                            }
                        }
                        if let Some(frame) = lifetime.take_gpu_presentation() {
                            let target = frame.target;
                            let presentation = frame.receipt;
                            displayed = Some(target);
                            lifetime.did_present_gpu(target)?;
                            if presentation.needs_follow_up {
                                lifetime.expose();
                            }
                            timing.poll(|| feedback::poll(lifetime, native.as_ref(), started.elapsed()))?;
                        }
                        if !picker.busy() && lifetime.take_picker_request() {
                            feedback::dispatch_input(lifetime, native.as_ref(), Input::Unfocused)?;
                            picker.request(SelectionKind::File)?;
                        }
                    }
                }
                _ => {}
            }
            if input_turn && !closing && options.visible && window.is_focused() {
                timing.activity();
                lifetime.expose();
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
        timing.turn(input_turn, turn_started.elapsed());
        timing.report(started.elapsed(), native.as_ref().map(NativeInput::take_input_delay).unwrap_or_default(), false);
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
    timing.report(
        started.elapsed(),
        native.as_ref().map(NativeInput::take_input_delay).unwrap_or_default(),
        true,
    );
    picker.retire()?;
    drop(picker);
    wake.disconnect();
    retirement::destroy(&mut events, window, native.as_ref())?;
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
