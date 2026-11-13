// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Pump native callbacks while preparation and owned shutdown progress without UI waits.

#[cfg(any(target_os = "linux", test))]
#[path = "linux_callback_failure.rs"]
pub(crate) mod callback_failure;
use crate::native_surface::{callback_kind, input, presentation, state, surface_shape};

use crate::cadence::{advance_deadline, closing_wait, presentation_wait};
use crate::completion_wake as wake;
#[cfg(target_os = "linux")]
use crate::linux::Presenter;
#[cfg(target_os = "macos")]
use crate::macos::Presenter;
use crate::native_action::NativeAction;
use crate::timing::{NativeStartupStage as StartupStage, observe_startup};
#[cfg(target_os = "windows")]
use crate::windows::Presenter;
use crate::{DesktopError, DesktopOptions, DesktopReceipt, Pointer, WindowContent};
use std::{num::NonZeroUsize, rc::Rc, sync::Arc, time::Instant};
use tao::{
    dpi::{LogicalSize, PhysicalPosition},
    event::Event,
    event_loop::{ControlFlow, EventLoopBuilder},
    platform::run_return::EventLoopExtRunReturn,
    window::{Theme, WindowBuilder},
};

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
    let timings = crate::timing::TimingObserver::native(
        options.audit_timings,
        crate::timing::NativeStartupOrder::SurfaceFirst,
    )?;
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
    #[cfg(target_os = "windows")]
    let mut apartment = crate::windows_ui_apartment::UiApartment::enter()?;
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
        // tao's GTK backend reports no outer size until the first configure event; this
        // undecorated window's inner size is its outer size and is known at creation.
        #[cfg(target_os = "windows")]
        let client = window.outer_size();
        #[cfg(not(target_os = "windows"))]
        let client = window.inner_size();
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
        &crate::windows_controls::Specification::buttons(content.accessible_controls()),
        crate::windows_controls::PaintMode::PreparedPixels,
        timings.clone(),
    )?;
    #[cfg(target_os = "windows")]
    let controls_initialization = controls.initialize();
    #[cfg(not(target_os = "windows"))]
    let controls_initialization: Result<(), DesktopError> = Ok(());
    let controls_initialization = controls_initialization
        .and_then(|()| observe_startup(timings.as_ref(), StartupStage::ControlsReady));
    let drain_budget = NonZeroUsize::new(32).ok_or_else(|| {
        DesktopError::message("start_content", "completion drain budget must be positive")
    })?;
    let wake_lifetime = wake::WakeLifetime::new();
    let completion_wake = Arc::new(wake::ProxyWake::new(
        event_loop.create_proxy(),
        wake_lifetime.clone(),
        NativeAction::Completions,
    ));
    let mut surface = state::SurfaceState::new(timings.clone());
    if let Err(cause) = controls_initialization {
        surface.fail(&mut content, cause);
    } else if let Err(error) = content.start(completion_wake) {
        surface.fail(&mut content, DesktopError::boxed("start_content", error));
    } else if let Err(cause) = observe_startup(timings.as_ref(), StartupStage::ExecutionReady) {
        surface.fail(&mut content, cause);
    }
    let mut presentations = presentation::Presentations::new(started, options.audit_frames);
    let mut pointer = Pointer::default();
    let mut pressed = None;
    let mut next_preparation = started;
    #[cfg(target_os = "windows")]
    let mut animation_period = crate::windows_animation_period::WindowsAnimationPeriod::new();
    loop {
        let exit_code = event_loop.run_return(|event, _, control| {
            let began = match timings.as_ref().map(crate::timing::TimingObserver::mark).transpose()
            {
                Ok(began) => began,
                Err(cause) => {
                    surface.fail(&mut content, cause);
                    None
                }
            };
            if let Some(cause) =
                timings.as_ref().and_then(crate::timing::TimingObserver::take_failure)
            {
                surface.fail(&mut content, cause);
            }
            let kind = callback_kind(&event);
            let observation =
                crate::timing::observe_callback(timings.as_ref(), began, kind, || {
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
                    let visible = (window.is_visible() || presentations.preparing_first())
                        && !window.is_minimized()
                        && actual_shape.is_some();
                    surface.visibility(&mut content, visible);
                    match event {
                        Event::WindowEvent { window_id, event, .. }
                            if window_id == window.id() && !surface.closing =>
                        {
                            match input::dispatch(
                                &mut content,
                                &window,
                                event,
                                &mut pointer,
                                &mut pressed,
                            ) {
                                Ok(effect) => {
                                    if effect.close {
                                        surface.close_by_user(&mut content);
                                    }
                                    #[cfg(windows)]
                                    if let Some(backward) = effect.focus
                                        && let Err(cause) = controls.focus_next(backward)
                                    {
                                        surface.fail(&mut content, cause);
                                    }
                                    if effect.dirty {
                                        surface.dirty();
                                    }
                                    if effect.expose && !surface.closing {
                                        window.request_redraw();
                                    }
                                }
                                Err(error) => surface.fail(&mut content, error),
                            }
                        }
                        Event::UserEvent(NativeAction::Completions) => {
                            if surface.poll(
                                &mut content,
                                actual_shape,
                                drain_budget,
                                started.elapsed(),
                            ) {
                                window.request_redraw();
                            }
                        }
                        #[cfg(target_os = "windows")]
                        Event::UserEvent(NativeAction::Close) => {
                            surface.close_by_user(&mut content)
                        }
                        #[cfg(target_os = "windows")]
                        Event::UserEvent(NativeAction::Activate(id)) if !surface.closing => {
                            if content.activate(id) {
                                surface.dirty();
                            }
                        }
                        #[cfg(target_os = "windows")]
                        Event::UserEvent(NativeAction::FocusNext(backward)) if !surface.closing => {
                            if let Err(cause) = controls.focus_next(backward) {
                                surface.fail(&mut content, cause);
                            }
                        }
                        #[cfg(target_os = "windows")]
                        Event::UserEvent(NativeAction::FocusChanged) if !surface.closing => {
                            window.request_redraw();
                        }
                        Event::RedrawRequested(window_id) if window_id == window.id() => {
                            if let Some(frame) = surface.exposable(actual_shape)
                                && let Err(error) = presentations.submit(
                                    &window,
                                    &mut presenter,
                                    frame,
                                    timings.as_ref(),
                                    &mut content,
                                    #[cfg(target_os = "windows")]
                                    &mut controls,
                                )
                            {
                                surface.fail(&mut content, error);
                            }
                        }
                        Event::MainEventsCleared => {
                            if surface.poll(
                                &mut content,
                                actual_shape,
                                drain_budget,
                                started.elapsed(),
                            ) {
                                window.request_redraw();
                            }
                            if presentations.preparing_first()
                                && let Some(frame) = surface.exposable(actual_shape)
                                && let Err(cause) = presentations.submit(
                                    &window,
                                    &mut presenter,
                                    frame,
                                    timings.as_ref(),
                                    &mut content,
                                    #[cfg(target_os = "windows")]
                                    &mut controls,
                                )
                            {
                                surface.fail(&mut content, cause);
                            }
                            if surface.closing {
                                surface.poll_shutdown(&mut content);
                            } else {
                                let now = Instant::now();
                                let due = surface.visible
                                    && content.animating()
                                    && now >= next_preparation;
                                surface.prepare(&mut content, actual_shape, started.elapsed(), due);
                                if due {
                                    next_preparation = advance_deadline(next_preparation, now);
                                }
                            }
                        }
                        _ => {}
                    }
                    if !surface.closing {
                        if let Err(cause) = presentations.reconcile(
                            &mut content,
                            &window,
                            &presenter,
                            timings.as_ref(),
                        ) {
                            surface.fail(&mut content, cause);
                        }
                        if content.close_requested() {
                            surface.close(&mut content);
                        }
                    }
                    #[cfg(target_os = "windows")]
                    if let Err(cause) = animation_period
                        .reconcile(surface.visible && content.animating() && !surface.closing)
                    {
                        surface.fail(&mut content, cause);
                    }
                    #[cfg(target_os = "windows")]
                    if surface.closing
                        && let Err(cause) = controls.shutdown()
                    {
                        surface.fail(&mut content, cause);
                    }
                    #[cfg(target_os = "windows")]
                    let native_retired = controls.retired();
                    #[cfg(not(target_os = "windows"))]
                    let native_retired = true;
                    *control = if surface.settled && native_retired {
                        ControlFlow::Exit
                    } else if surface.closing {
                        closing_wait(Instant::now())
                    } else {
                        presentation_wait(
                            surface.visible,
                            false,
                            content.animating().then_some(next_preparation),
                            shutdown_deadline.into_iter().chain(content.activity_deadline()).min(),
                        )
                    };
                });
            if let Err(cause) = observation {
                surface.fail(&mut content, cause);
                #[cfg(target_os = "windows")]
                if let Err(cause) = animation_period.reconcile(false) {
                    surface.fail(&mut content, cause);
                }
                *control = if surface.settled && {
                    #[cfg(target_os = "windows")]
                    {
                        controls.retired()
                    }
                    #[cfg(not(target_os = "windows"))]
                    {
                        true
                    }
                } {
                    ControlFlow::Exit
                } else {
                    closing_wait(Instant::now())
                };
            }
        });
        #[cfg(target_os = "windows")]
        let native_retired = controls.retired();
        #[cfg(not(target_os = "windows"))]
        let native_retired = true;
        if surface.settled && native_retired {
            if exit_code != 0 {
                DesktopError::retain_first(
                    &mut surface.failure,
                    DesktopError::message(
                        "native_surface_loop",
                        format!("Native loop returned status {exit_code}"),
                    ),
                );
            }
            break;
        }
        surface.fail(
            &mut content,
            DesktopError::message(
                "native_surface_loop",
                format!("Native loop returned before actual settlement (status {exit_code})"),
            ),
        );
    }
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
    crate::shutdown::finish_window_content(
        content,
        || {
            #[cfg(target_os = "windows")]
            {
                // SAFETY: Controls, presenter, window and loop have retired above;
                // finish_window_content released the settled content before this call.
                #[allow(unsafe_code)]
                unsafe {
                    apartment.release_after_teardown()
                }
            }
            #[cfg(not(target_os = "windows"))]
            Ok(())
        },
        &mut surface.failure,
    );
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
