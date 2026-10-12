// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own the browser-independent Windows report loop and checked teardown.
mod buffer;
mod callback;
mod clipboard;
mod controls;
mod font;
mod save_dialog;
mod session;
use super::{ReportSessionContent, ReportSurfaceError, ReportSurfaceOptions, ReportSurfaceReceipt};
use callback::{Event as WakeEvent, Input};
use std::{rc::Rc, sync::Arc, time::Instant};
use sugarsubstitute_contracts::incident::ReportAction;
use tao::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    platform::run_return::EventLoopExtRunReturn,
    window::{Theme, WindowBuilder},
};
/// Run one owned native reporter after authenticated bootstrap, independently of webviews.
/// Content calls execute promptly on this thread; jobs use the existing execution owner.
/// # Errors
/// Preserve causes and retain uncertain resources. The original fifteen-second shutdown
/// bound grants no settlement or success receipt.
pub fn run_report_surface<T: ReportSessionContent + 'static>(
    options: ReportSurfaceOptions,
    mut content: T,
) -> Result<ReportSurfaceReceipt, ReportSurfaceError> {
    let (dark, width, height, lease, title) =
        (options.dark, options.width, options.height, options.lease.clone(), options.labels.title);
    let mut event_loop = EventLoopBuilder::<WakeEvent>::with_user_event().build();
    let lifetime = crate::completion_wake::WakeLifetime::new();
    let wake = Arc::new(crate::completion_wake::ProxyWake::new(
        event_loop.create_proxy(),
        lifetime.clone(),
        WakeEvent::Wake,
    ));
    let mut session = session::Session::new(options);
    let mut apartment = None;
    let initialization = content.start(wake).and_then(|()| {
        apartment = Some(crate::windows_ui_apartment::UiApartment::enter()?);
        let window = WindowBuilder::new()
            .with_title(title)
            .with_visible(false)
            .with_theme(Some(if dark { Theme::Dark } else { Theme::Light }))
            .with_inner_size(LogicalSize::new(width, height))
            .with_min_inner_size(LogicalSize::new(360, 280))
            .build(&event_loop)
            .map_err(|cause| ReportSurfaceError::new("report_parent_create", cause))?;
        session.initialize(Rc::new(window), event_loop.create_proxy(), lease, &mut content)
    });
    if let Err(cause) = initialization {
        session.fail(&mut content, cause);
    }
    let mut expired = false;
    let exit_status = event_loop.run_return(|event, _, flow| {
        *flow = crate::cadence::closing_wait(Instant::now());
        match event {
            Event::WindowEvent { window_id, event, .. } if Some(window_id) == session.window_id => {
                match event {
                    WindowEvent::Destroyed => session.destroyed = true,
                    WindowEvent::CloseRequested => {
                        let input = Input {
                            binding: session.state.binding(),
                            intent: super::state::Intent::Action(ReportAction::Dismiss),
                        };
                        if let Err(cause) = session.input(&mut content, input) {
                            session.fail(&mut content, cause);
                        }
                    }
                    WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                        let result =
                            session.controls.as_mut().map_or(Ok(()), |controls| controls.layout());
                        if let Err(cause) = result {
                            session.fail(&mut content, cause);
                        }
                    }
                    _ => {}
                }
            }
            Event::MainEventsCleared => {
                if session.state.closing() {
                    session.shutdown(&mut content);
                    session.observe_retirement(&mut content);
                } else {
                    let mut remaining = 32;
                    let inputs = session.controls.as_ref().map(|controls| controls.callback.take());
                    if let Some(inputs) = inputs {
                        for input in inputs.into_iter().flatten() {
                            remaining -= 1;
                            if let Err(cause) = session.input(&mut content, input) {
                                session.fail(&mut content, cause);
                                break;
                            }
                        }
                    }
                    let failure = session
                        .controls
                        .as_ref()
                        .and_then(|controls| controls.callback.take_failure());
                    if let Some(cause) = failure {
                        session.fail(&mut content, cause);
                    }
                    if let Some(maximum) = std::num::NonZeroUsize::new(remaining)
                        && let Err(cause) = session.poll(&mut content, maximum)
                    {
                        session.fail(&mut content, cause);
                    }
                }
                if session.teardown_ready() {
                    *flow = ControlFlow::Exit;
                } else if session.close_deadline.is_some_and(|deadline| Instant::now() >= deadline)
                {
                    expired = true;
                    *flow = ControlFlow::Exit;
                }
            }
            _ => {}
        }
    });
    lifetime.disconnect();
    if !session.teardown_ready() {
        if !expired {
            session.fail(
                &mut content,
                ReportSurfaceError::new(
                    "report_native_loop_return",
                    std::io::Error::other(format!(
                        "Native loop returned with status {exit_status} before checked teardown"
                    )),
                ),
            );
        }
        // Preserve unresolved native registrations and jobs until process/thread termination.
        std::mem::forget(event_loop);
        std::mem::forget(session.controls.take());
        std::mem::forget(session.window.take());
        std::mem::forget(session.picker.take());
        std::mem::forget(apartment);
        std::mem::forget(content);
        return Err(session
            .failure
            .unwrap_or_else(|| ReportSurfaceError::rejected("report_native_shutdown_deadline")));
    }
    drop(event_loop);
    if exit_status != 0 {
        session.retain(ReportSurfaceError::new(
            "report_native_loop_status",
            std::io::Error::other(format!("Native loop returned with status {exit_status}")),
        ));
    }
    let mut final_failure = None;
    crate::shutdown::finish_after_native(
        content,
        |content| {
            content
                .finish_shutdown()
                .map_err(|cause| crate::DesktopError::new("finish_report_shutdown", cause))
        },
        || {
            if let Some(apartment) = &mut apartment {
                // SAFETY: Actual native holders/loop retired; existing owner finished and dropped
                // the actual content while this same UI STA reference stayed held.
                unsafe { apartment.release_after_teardown() }
            } else {
                Ok(())
            }
        },
        &mut final_failure,
    );
    if let Some(cause) = final_failure {
        session.retain(cause.into());
    }
    if let Some(cause) = session.failure {
        return Err(cause);
    }
    session.state.receipt()
}
