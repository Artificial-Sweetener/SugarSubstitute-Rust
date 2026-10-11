// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose one native event loop, borrowed surface presenter, and replaceable content.

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
    event_loop::{ControlFlow, EventLoop},
    keyboard::Key,
    platform::run_return::EventLoopExtRunReturn,
    window::{Theme, WindowBuilder},
};

/// Match the reference animation cadence without adding raster time to each period.
const FRAME_INTERVAL: Duration = Duration::from_millis(16);

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
    let mut event_loop = EventLoop::new();
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
    window.request_redraw();
    event_loop.run_return(|event, _, control| {
        *control = ControlFlow::WaitUntil(next_frame);
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
                        content.pointer(PointerAction::Move, pointer);
                        window.request_redraw();
                    }
                    WindowEvent::CursorLeft { .. } => {
                        pressed = None;
                        content.pointer(PointerAction::Leave, pointer);
                        window.request_redraw();
                    }
                    WindowEvent::Focused(false) => {
                        pressed = None;
                        content.pointer(PointerAction::Leave, pointer);
                        window.request_redraw();
                    }
                    WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
                        let target = content.hit_test(pointer);
                        if state == ElementState::Pressed {
                            pressed = Some(target);
                            content.pointer(PointerAction::Press, pointer);
                            if target == HitTarget::Caption
                                && let Err(error) = window.drag_window()
                            {
                                failure = Some(DesktopError::new("drag_window", error));
                                *control = ControlFlow::Exit;
                            }
                        } else {
                            content.pointer(PointerAction::Release, pointer);
                            if pressed == Some(HitTarget::Close) && target == HitTarget::Close {
                                *control = ControlFlow::Exit;
                            }
                            pressed = None;
                        }
                        window.request_redraw();
                    }
                    WindowEvent::KeyboardInput { event, .. }
                        if event.state == ElementState::Pressed && !event.repeat =>
                    {
                        match event.logical_key {
                            Key::Escape => *control = ControlFlow::Exit,
                            Key::Tab => {
                                content.key(KeyAction::Tab);
                                window.request_redraw();
                            }
                            _ => {}
                        }
                    }
                    WindowEvent::Resized(_)
                    | WindowEvent::ScaleFactorChanged { .. }
                    | WindowEvent::Focused(true) => window.request_redraw(),
                    _ => {}
                }
            }
            Event::RedrawRequested(window_id)
                if window_id == window.id() && window.is_visible() && !window.is_minimized() =>
            {
                let size = window.inner_size();
                if size.width > 0 && size.height > 0 {
                    let frame_started = Instant::now();
                    let result = content
                        .render(size.width, size.height, window.scale_factor(), started.elapsed())
                        .map_err(|error| DesktopError::boxed("render_frame", error))
                        .and_then(|frame| {
                            frame.validate()?;
                            presenter.present(&frame)
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
                    && window.is_visible()
                    && !window.is_minimized() =>
            {
                window.request_redraw()
            }
            _ => {}
        }
        if !matches!(*control, ControlFlow::ExitWithCode(_)) {
            *control = presentation_wait(
                window.is_visible(),
                window.is_minimized(),
                next_frame,
                shutdown_deadline,
            );
        }
    });
    let material = presenter.material();
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

/// Suspend hidden native surfaces without losing the qualification shutdown deadline.
fn presentation_wait(
    visible: bool,
    minimized: bool,
    next_frame: Instant,
    shutdown: Option<Instant>,
) -> ControlFlow {
    if visible && !minimized {
        ControlFlow::WaitUntil(next_frame)
    } else if let Some(deadline) = shutdown {
        ControlFlow::WaitUntil(deadline)
    } else {
        ControlFlow::Wait
    }
}

/// Skip missed periods while preserving cadence; an early interaction keeps the next deadline.
fn advance_deadline(deadline: Instant, now: Instant) -> Instant {
    if now < deadline {
        return deadline;
    }
    let periods = now.duration_since(deadline).as_nanos() / FRAME_INTERVAL.as_nanos() + 1;
    u32::try_from(periods)
        .ok()
        .and_then(|periods| FRAME_INTERVAL.checked_mul(periods))
        .and_then(|delay| deadline.checked_add(delay))
        .unwrap_or_else(|| now + FRAME_INTERVAL)
}

#[cfg(test)]
mod tests {
    use super::{FRAME_INTERVAL, advance_deadline, presentation_wait};
    use std::time::{Duration, Instant};
    use tao::event_loop::ControlFlow;

    /// Keep raster work from accumulating drift and skip missed frames instead of busy catching up.
    #[test]
    fn cadence_skips_late_frames_and_keeps_early_input_deadline() {
        let origin = Instant::now();
        let scheduled = origin + FRAME_INTERVAL;
        assert_eq!(advance_deadline(scheduled, origin + Duration::from_millis(4)), scheduled);
        assert_eq!(
            advance_deadline(scheduled, origin + Duration::from_millis(27)),
            origin + Duration::from_millis(32)
        );
        assert_eq!(
            advance_deadline(scheduled, origin + Duration::from_millis(48)),
            origin + Duration::from_millis(64)
        );
    }

    /// Stop background animation work while preserving a bounded native run's eventual exit.
    #[test]
    fn hidden_surfaces_wait_for_native_events_or_shutdown() {
        let now = Instant::now();
        let next = now + FRAME_INTERVAL;
        let shutdown = now + Duration::from_secs(3);
        assert_eq!(
            presentation_wait(false, false, next, Some(shutdown)),
            ControlFlow::WaitUntil(shutdown)
        );
        assert_eq!(
            presentation_wait(true, true, next, Some(shutdown)),
            ControlFlow::WaitUntil(shutdown)
        );
        assert_eq!(presentation_wait(false, false, next, None), ControlFlow::Wait);
        assert_eq!(
            presentation_wait(true, false, next, Some(shutdown)),
            ControlFlow::WaitUntil(next)
        );
    }
}
