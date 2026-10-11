// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose one native event loop, borrowed surface presenter, and replaceable content.

use crate::cadence::{advance_deadline, presentation_wait};
#[cfg(target_os = "linux")]
use crate::linux::Presenter;
#[cfg(target_os = "macos")]
use crate::macos::Presenter;
#[cfg(target_os = "windows")]
use crate::windows::Presenter;
use crate::{
    DesktopError, DesktopOptions, DesktopReceipt, HitTarget, KeyAction, Pointer, PointerAction,
    WindowContent,
};
use std::{
    rc::Rc,
    time::{Duration, Instant},
};
use tao::{
    dpi::{LogicalSize, PhysicalPosition},
    event::{ElementState, Event, MouseButton, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    keyboard::Key,
    platform::run_return::EventLoopExtRunReturn,
    window::{Theme, WindowBuilder},
};

/// Forward native peer activation without lending callbacks the content owner.
#[derive(Clone, Copy, Debug)]
pub(crate) enum NativeAction {
    /// Activate a surface-local control.
    #[cfg(target_os = "windows")]
    Activate(u16),
    /// Close the surface through the same native terminal path as pointer/Escape.
    #[cfg(target_os = "windows")]
    Close,
    /// Preserve the demonstration's existing disclosure shortcut.
    #[cfg(target_os = "windows")]
    Disclosure,
    /// Repaint a changed native focus cue over a fresh CPU frame.
    #[cfg(target_os = "windows")]
    FocusChanged,
}

/// Run a native surface on the calling main thread until closed or its qualification deadline.
///
/// The content owns presentation state. This adapter never launches processes, negotiates
/// startup, or depends on a particular splash. Errors close the owned surface and preserve causes.
pub fn run(
    options: DesktopOptions,
    mut content: impl WindowContent,
) -> Result<DesktopReceipt, DesktopError> {
    if options.logical_width == 0 || options.logical_height == 0 {
        return Err(DesktopError::message("create_window", "client dimensions must be positive"));
    }
    let started = Instant::now();
    let shutdown_deadline = match options.duration {
        Some(duration) => Some(started.checked_add(duration).ok_or_else(|| {
            DesktopError::message(
                "create_window",
                "qualification duration exceeds the native clock range",
            )
        })?),
        None => None,
    };
    let mut event_loop = EventLoopBuilder::<NativeAction>::with_user_event().build();
    let window = Rc::new(
        WindowBuilder::new()
            .with_title(options.title)
            .with_inner_size(LogicalSize::new(options.logical_width, options.logical_height))
            .with_decorations(false)
            .with_resizable(false)
            .with_transparent(cfg!(target_os = "windows"))
            .with_theme(Some(if options.dark { Theme::Dark } else { Theme::Light }))
            .with_visible(false)
            .build(&event_loop)
            .map_err(|error| DesktopError::new("create_window", error))?,
    );
    if let Some(monitor) = window.current_monitor().or_else(|| window.primary_monitor()) {
        let origin = monitor.position();
        let size = monitor.size();
        let client = window.outer_size();
        let x = i64::from(origin.x) + (i64::from(size.width) - i64::from(client.width)) / 2;
        let y = i64::from(origin.y) + (i64::from(size.height) - i64::from(client.height)) / 2;
        if let (Ok(x), Ok(y)) = (i32::try_from(x), i32::try_from(y)) {
            window.set_outer_position(PhysicalPosition::new(x, y));
        }
    }
    let mut presenter = Presenter::new(Rc::clone(&window), options.dark)?;
    #[cfg(target_os = "windows")]
    let mut controls = crate::windows_controls::Controls::new(
        Rc::clone(&window),
        event_loop.create_proxy(),
        content.accessible_controls(),
    )?;
    let mut first_frame = None;
    let mut frames_presented = 0;
    let mut total_frame_work = Duration::ZERO;
    let mut total_frame_intervals = Duration::ZERO;
    let mut longest_frame_interval = Duration::ZERO;
    let mut last_submission = None::<Instant>;
    let mut pointer = Pointer::default();
    let mut pressed = None;
    let mut failure = None;
    let mut next_frame = started;
    window.set_visible(true);
    window.set_focus();
    let mut redraw_pending = false;
    invalidate(&window, &mut redraw_pending);
    event_loop.run_return(|event, _, control| {
        *control = ControlFlow::Wait;
        if options.duration.is_some_and(|duration| started.elapsed() >= duration) {
            *control = ControlFlow::Exit;
            return;
        }
        match event {
            Event::WindowEvent { window_id, event, .. } if window_id == window.id() => {
                match event {
                    WindowEvent::CloseRequested => *control = ControlFlow::Exit,
                    WindowEvent::CursorMoved { position, .. } => {
                        let scale = window.scale_factor();
                        pointer = Pointer { x: position.x / scale, y: position.y / scale };
                        if content.pointer(PointerAction::Move, pointer) {
                            invalidate(&window, &mut redraw_pending);
                        }
                    }
                    WindowEvent::CursorLeft { .. } => {
                        pressed = None;
                        if content.pointer(PointerAction::Leave, pointer) {
                            invalidate(&window, &mut redraw_pending);
                        }
                    }
                    WindowEvent::Focused(false) => {
                        pressed = None;
                        if content.pointer(PointerAction::Leave, pointer) {
                            invalidate(&window, &mut redraw_pending);
                        }
                    }
                    WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
                        let target = content.hit_test(pointer);
                        if state == ElementState::Pressed {
                            pressed = Some(target);
                            if content.pointer(PointerAction::Press, pointer) {
                                invalidate(&window, &mut redraw_pending);
                            }
                            if target == HitTarget::Caption {
                                #[cfg(target_os = "windows")]
                                let drag = crate::windows_drag::begin(&window);
                                #[cfg(not(target_os = "windows"))]
                                let drag = window
                                    .drag_window()
                                    .map_err(|error| DesktopError::new("drag_window", error));
                                match drag {
                                    Ok(()) => {
                                        pressed = None;
                                        if content.pointer(PointerAction::Leave, pointer) {
                                            invalidate(&window, &mut redraw_pending);
                                        }
                                    }
                                    Err(error) => {
                                        failure = Some(error);
                                        *control = ControlFlow::Exit;
                                    }
                                }
                            }
                        } else {
                            if content.pointer(PointerAction::Release, pointer) {
                                invalidate(&window, &mut redraw_pending);
                            }
                            if pressed == Some(HitTarget::Close) && target == HitTarget::Close {
                                *control = ControlFlow::Exit;
                            }
                            pressed = None;
                        }
                    }
                    WindowEvent::KeyboardInput { event, .. }
                        if event.state == ElementState::Pressed && !event.repeat =>
                    {
                        match event.logical_key {
                            Key::Escape => *control = ControlFlow::Exit,
                            Key::Tab => {
                                if content.key(KeyAction::Tab) {
                                    invalidate(&window, &mut redraw_pending);
                                }
                            }
                            _ => {}
                        }
                    }
                    WindowEvent::Resized(_)
                    | WindowEvent::ScaleFactorChanged { .. }
                    | WindowEvent::Focused(true) => invalidate(&window, &mut redraw_pending),
                    _ => {}
                }
            }
            Event::UserEvent(action) => match action {
                #[cfg(target_os = "windows")]
                NativeAction::Activate(id) => {
                    if content.activate(id) {
                        invalidate(&window, &mut redraw_pending);
                    }
                }
                #[cfg(target_os = "windows")]
                NativeAction::Close => *control = ControlFlow::Exit,
                #[cfg(target_os = "windows")]
                NativeAction::Disclosure => {
                    if content.key(KeyAction::Tab) {
                        invalidate(&window, &mut redraw_pending);
                    }
                }
                #[cfg(target_os = "windows")]
                NativeAction::FocusChanged => invalidate(&window, &mut redraw_pending),
            },
            Event::RedrawRequested(window_id) if window_id == window.id() => {
                redraw_pending = false;
                let size = window.inner_size();
                if window.is_visible()
                    && !window.is_minimized()
                    && size.width > 0
                    && size.height > 0
                {
                    let frame_started = Instant::now();
                    let result = content
                        .render(size.width, size.height, window.scale_factor(), started.elapsed())
                        .map_err(|error| DesktopError::boxed("render_frame", error))
                        .and_then(|frame| {
                            frame.validate()?;
                            presenter.present(&frame)?;
                            #[cfg(target_os = "windows")]
                            {
                                controls.update(content.accessible_controls())?;
                                controls.draw_focus()?;
                            }
                            Ok(())
                        });
                    match result {
                        Ok(()) => {
                            let submitted = Instant::now();
                            total_frame_work += submitted.duration_since(frame_started);
                            if let Some(previous) = last_submission {
                                let interval = submitted.duration_since(previous);
                                total_frame_intervals += interval;
                                longest_frame_interval = longest_frame_interval.max(interval);
                            }
                            last_submission = Some(submitted);
                            if first_frame.is_none() {
                                let elapsed = started.elapsed();
                                tracing::info!(
                                    operation = "native_first_frame",
                                    elapsed_us = elapsed.as_micros(),
                                    "Submitted the first CPU frame"
                                );
                                first_frame = Some(elapsed);
                            }
                            frames_presented += 1;
                        }
                        Err(error) => {
                            failure = Some(error);
                            *control = ControlFlow::Exit;
                        }
                    }
                }
                next_frame = advance_deadline(next_frame, Instant::now());
            }
            Event::MainEventsCleared
                if Instant::now() >= next_frame
                    && content.animating()
                    && window.is_visible()
                    && !window.is_minimized() =>
            {
                invalidate(&window, &mut redraw_pending)
            }
            _ => {}
        }
        if !matches!(*control, ControlFlow::ExitWithCode(_)) {
            *control = presentation_wait(
                window.is_visible(),
                window.is_minimized(),
                content.animating().then_some(next_frame),
                shutdown_deadline,
            );
        }
    });
    let material = presenter.material();
    #[cfg(target_os = "windows")]
    if let Err(error) = controls.shutdown() {
        failure.get_or_insert(error);
    }
    #[cfg(target_os = "windows")]
    drop(controls);
    drop(presenter);
    drop(window);
    if let Some(error) = failure {
        return Err(error);
    }
    let first_frame = first_frame.ok_or_else(|| {
        DesktopError::message("present_frame", "window closed before the first frame")
    })?;
    Ok(DesktopReceipt {
        first_frame,
        frames_presented,
        material,
        total_frame_work,
        total_frame_intervals,
        longest_frame_interval,
    })
}

/// Coalesce meaningful input invalidations until the native redraw is consumed.
fn invalidate(window: &tao::window::Window, pending: &mut bool) {
    if !*pending {
        *pending = true;
        window.request_redraw();
    }
}
