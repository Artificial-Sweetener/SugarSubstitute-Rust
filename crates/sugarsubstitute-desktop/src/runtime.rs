// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Pump native callbacks while preparation and owned shutdown progress without UI waits.

#[cfg(any(target_os = "linux", test))]
#[path = "linux_callback_failure.rs"]
pub(crate) mod callback_failure;
#[path = "runtime_input.rs"]
mod input;
#[path = "runtime_present.rs"]
mod presentation;
#[path = "runtime_state.rs"]
mod state;
#[path = "runtime_wake.rs"]
mod wake;

#[cfg(target_os = "windows")]
use crate::KeyAction;
use crate::cadence::{advance_deadline, closing_wait, presentation_wait};
#[cfg(target_os = "linux")]
use crate::linux::Presenter;
#[cfg(target_os = "macos")]
use crate::macos::Presenter;
use crate::timing::{NativeStartupStage as StartupStage, observe_startup};
#[cfg(target_os = "windows")]
use crate::windows::Presenter;
use crate::{DesktopError, DesktopOptions, DesktopReceipt, Pointer, WindowContent};
use std::{num::NonZeroUsize, rc::Rc, sync::Arc, time::Instant};
use sugarsubstitute_contracts::presentation::FrameShape;
use tao::{
    dpi::{LogicalSize, PhysicalPosition},
    event::Event,
    event_loop::{ControlFlow, EventLoopBuilder},
    platform::run_return::EventLoopExtRunReturn,
    window::{Theme, WindowBuilder},
};

/// Queue native peer intent without lending callbacks content or scheduling handles.
#[derive(Clone, Copy, Debug)]
pub(crate) enum NativeAction {
    /// Bounded completion metadata is available, including during owned close.
    Completions,
    /// Activate a surface-local control.
    #[cfg(target_os = "windows")]
    Activate(u16),
    /// Request the shared owned close protocol.
    #[cfg(target_os = "windows")]
    Close,
    /// Forward the content disclosure shortcut.
    #[cfg(target_os = "windows")]
    Disclosure,
    /// Repaint the retained source to restore a changed native focus cue.
    #[cfg(target_os = "windows")]
    FocusChanged,
}

/// Run a main-thread native surface through actual owned shutdown and native teardown.
///
/// Preparation is submitted and polled through content; redraw consumes immutable
/// validated storage. Closing disables admission/publication/input, continues bounded
/// terminal disposal and native callbacks, and exits only after actual settlement.
/// The first error is retained while shutdown proceeds. Final owner teardown occurs
/// after controls, presenter, window and event loop have all been dropped.
pub fn run(
    options: DesktopOptions,
    mut content: impl WindowContent,
) -> Result<DesktopReceipt, DesktopError> {
    if options.logical_width == 0 || options.logical_height == 0 {
        return Err(DesktopError::message("create_window", "client dimensions must be positive"));
    }
    let timings = crate::timing::TimingObserver::native(options.audit_timings)?;
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
    observe_startup(timings.as_ref(), StartupStage::EventLoopReady)?;
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
    observe_startup(timings.as_ref(), StartupStage::WindowCreated)?;
    let mut presenter = Presenter::new(Rc::clone(&window), options.dark)?;
    observe_startup(timings.as_ref(), StartupStage::PresenterReady)?;
    #[cfg(target_os = "windows")]
    let mut controls = crate::windows_controls::Controls::new(
        Rc::clone(&window),
        event_loop.create_proxy(),
        content.accessible_controls(),
        timings.clone(),
    )?;
    observe_startup(timings.as_ref(), StartupStage::ControlsReady)?;
    let drain_budget = NonZeroUsize::new(32).ok_or_else(|| {
        DesktopError::message("start_content", "completion drain budget must be positive")
    })?;
    let wake_lifetime = wake::WakeLifetime::new();
    let completion_wake =
        Arc::new(wake::ProxyWake::new(event_loop.create_proxy(), wake_lifetime.clone()));
    let mut surface = state::SurfaceState::new(timings.clone());
    if let Err(error) = content.start(completion_wake) {
        surface.fail(&mut content, DesktopError::boxed("start_content", error));
    } else if let Err(cause) = observe_startup(timings.as_ref(), StartupStage::ExecutionReady) {
        surface.fail(&mut content, cause);
    }
    let mut presentations = presentation::Presentations::new(started, options.audit_frames);
    let mut pointer = Pointer::default();
    let mut pressed = None;
    let mut next_preparation = started;
    let mut redraw_pending = false;
    #[cfg(target_os = "windows")]
    let mut animation_period = crate::windows_animation_period::WindowsAnimationPeriod::new();
    if !surface.closing {
        window.set_visible(true);
        window.set_focus();
        if let Err(cause) = observe_startup(timings.as_ref(), StartupStage::ShowFocusReturned) {
            surface.fail(&mut content, cause);
        }
    }
    event_loop.run_return(|event, _, control| {
        let began = match timings.as_ref().map(crate::timing::TimingObserver::mark).transpose() {
            Ok(began) => began,
            Err(cause) => {
                surface.fail(&mut content, cause);
                None
            }
        };
        if let Some(cause) = timings.as_ref().and_then(crate::timing::TimingObserver::take_failure)
        {
            surface.fail(&mut content, cause);
        }
        let kind = callback_kind(&event);
        let observation = crate::timing::observe_callback(timings.as_ref(), began, kind, || {
            #[cfg(target_os = "linux")]
            if let Err(error) = presenter.check_callbacks() {
                surface.fail(&mut content, error);
            }
            if options.duration.is_some_and(|duration| started.elapsed() >= duration) {
                surface.close(&mut content);
            }
            let actual_shape = match surface_shape(&window) {
                Ok(shape) => shape,
                Err(error) => {
                    surface.fail(&mut content, error);
                    None
                }
            };
            let visible = window.is_visible() && !window.is_minimized() && actual_shape.is_some();
            surface.visibility(&mut content, visible);
            match event {
                Event::WindowEvent { window_id, event, .. }
                    if window_id == window.id() && !surface.closing =>
                {
                    match input::dispatch(&mut content, &window, event, &mut pointer, &mut pressed)
                    {
                        Ok(effect) => {
                            if effect.close {
                                surface.close(&mut content);
                            }
                            if effect.dirty {
                                surface.dirty();
                            }
                            if effect.expose && !surface.closing {
                                invalidate(&window, &mut redraw_pending);
                            }
                        }
                        Err(error) => surface.fail(&mut content, error),
                    }
                }
                Event::UserEvent(NativeAction::Completions) => {
                    if surface.poll(&mut content, actual_shape, drain_budget, started.elapsed()) {
                        invalidate(&window, &mut redraw_pending);
                    }
                }
                #[cfg(target_os = "windows")]
                Event::UserEvent(NativeAction::Close) => surface.close(&mut content),
                #[cfg(target_os = "windows")]
                Event::UserEvent(NativeAction::Activate(id)) if !surface.closing => {
                    if content.activate(id) {
                        surface.dirty();
                    }
                }
                #[cfg(target_os = "windows")]
                Event::UserEvent(NativeAction::Disclosure) if !surface.closing => {
                    if content.key(KeyAction::Tab) {
                        surface.dirty();
                    }
                }
                #[cfg(target_os = "windows")]
                Event::UserEvent(NativeAction::FocusChanged) if !surface.closing => {
                    invalidate(&window, &mut redraw_pending);
                }
                Event::RedrawRequested(window_id) if window_id == window.id() => {
                    redraw_pending = false;
                    if let Some(frame) = surface.exposable(actual_shape)
                        && let Err(error) = presentations.submit(
                            &window,
                            &mut presenter,
                            frame,
                            timings.as_ref(),
                            &content,
                            #[cfg(target_os = "windows")]
                            &mut controls,
                        )
                    {
                        surface.fail(&mut content, error);
                    }
                }
                Event::MainEventsCleared => {
                    if surface.closing {
                        surface.poll(&mut content, actual_shape, drain_budget, started.elapsed());
                        surface.poll_shutdown(&mut content);
                    } else {
                        let now = Instant::now();
                        let due = surface.visible && content.animating() && now >= next_preparation;
                        surface.prepare(&mut content, actual_shape, started.elapsed(), due);
                        if due {
                            next_preparation = advance_deadline(next_preparation, now);
                        }
                    }
                }
                _ => {}
            }
            #[cfg(target_os = "windows")]
            if let Err(cause) = animation_period
                .reconcile(surface.visible && content.animating() && !surface.closing)
            {
                surface.fail(&mut content, cause);
            }
            *control = if surface.settled {
                ControlFlow::Exit
            } else if surface.closing {
                closing_wait(Instant::now())
            } else {
                presentation_wait(
                    surface.visible,
                    false,
                    content.animating().then_some(next_preparation),
                    shutdown_deadline,
                )
            };
        });
        if let Err(cause) = observation {
            surface.fail(&mut content, cause);
            #[cfg(target_os = "windows")]
            if let Err(cause) = animation_period.reconcile(false) {
                surface.fail(&mut content, cause);
            }
            *control =
                if surface.settled { ControlFlow::Exit } else { closing_wait(Instant::now()) };
        }
    });
    #[cfg(target_os = "windows")]
    {
        if let Err(cause) = animation_period.shutdown() {
            DesktopError::retain_first(&mut surface.failure, cause);
        }
        drop(animation_period);
    }
    // No worker can still emit callbacks after settlement. Reject retained external
    // wake guards before beginning the ordered native teardown below.
    wake_lifetime.disconnect();
    let material = presenter.material();
    #[cfg(target_os = "windows")]
    if let Err(error) = controls.shutdown() {
        DesktopError::retain_first(&mut surface.failure, error);
    }
    #[cfg(target_os = "windows")]
    drop(controls);
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    if let Err(error) = presenter.shutdown() {
        DesktopError::retain_first(&mut surface.failure, error);
    }
    drop(presenter);
    drop(window);
    drop(event_loop);
    surface.retained = None;
    if let Err(error) = content.finish_shutdown() {
        DesktopError::retain_first(
            &mut surface.failure,
            DesktopError::boxed("finish_shutdown", error),
        );
    }
    let timing_receipt = match timings.map(crate::timing::TimingObserver::finish).transpose() {
        Ok(receipt) => receipt,
        Err(cause) => {
            DesktopError::retain_first(&mut surface.failure, cause);
            None
        }
    };
    if let Some(cause) = surface.failure {
        return Err(cause);
    }
    let mut receipt = presentations.finish(material)?;
    receipt.timings = timing_receipt;
    Ok(receipt)
}

/// Validate actual device shape without scanning or touching retained pixel storage.
fn surface_shape(window: &tao::window::Window) -> Result<Option<FrameShape>, DesktopError> {
    let size = window.inner_size();
    if size.width == 0 || size.height == 0 {
        return Ok(None);
    }
    FrameShape::new(size.width, size.height, window.scale_factor())
        .map(Some)
        .map_err(|cause| DesktopError::new("native_frame_shape", cause))
}

/// Query opt-in move evidence around submission; portable surfaces have no Windows evidence.
fn observe_move(window: &tao::window::Window, auditing: bool) -> Result<bool, DesktopError> {
    #[cfg(target_os = "windows")]
    if auditing {
        return crate::windows_move_state::is_moving(window);
    }
    #[cfg(not(target_os = "windows"))]
    let _ = (window, auditing);
    Ok(false)
}

/// Coalesce completion/expose redraws independently of preparation requests.
fn invalidate(window: &tao::window::Window, pending: &mut bool) {
    if !*pending {
        *pending = true;
        window.request_redraw();
    }
}

/// Classify callbacks without borrowing content or touching frame storage.
fn callback_kind(event: &Event<'_, NativeAction>) -> crate::timing::NativeCallbackKind {
    use crate::timing::NativeCallbackKind;
    match event {
        Event::WindowEvent { .. } => NativeCallbackKind::WindowEvent,
        Event::UserEvent(NativeAction::Completions) => NativeCallbackKind::CompletionDrain,
        Event::UserEvent(_) => NativeCallbackKind::NativeAction,
        Event::RedrawRequested(_) => NativeCallbackKind::Redraw,
        Event::MainEventsCleared => NativeCallbackKind::LoopProgress,
        _ => NativeCallbackKind::Other,
    }
}
