// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own the browser-independent Windows report loop and checked teardown.
mod buffer;
mod callback;
mod controls;
mod copy;
mod dispatch;
mod modal;
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
    platform::{run_return::EventLoopExtRunReturn, windows::EventLoopBuilderExtWindows},
    window::{Theme, WindowBuilder},
};
/// Run one owned native reporter after authenticated bootstrap, independently of webviews.
/// Content calls execute promptly on this thread; jobs use the existing execution owner.
/// # Errors
/// Preserve causes and retain uncertain resources. The original fifteen-second shutdown
/// bound grants no settlement or success receipt. Refuse oversized Session/content metadata
/// before start; the existing UI lease retains one bounded dispatch reservation.
pub fn run_report_surface<T: ReportSessionContent + 'static>(
    options: ReportSurfaceOptions,
    content: T,
) -> Result<ReportSurfaceReceipt, ReportSurfaceError> {
    let (dark, width, height, lease, title) =
        (options.dark, options.width, options.height, options.lease.clone(), options.labels.title);
    validate_content_capacity::<T>()?;
    let (session, storage) = session::Session::new(options);
    let owner = dispatch::SurfaceOwner::new(session, content, storage);
    let native_owner = owner.clone();
    let mut builder = EventLoopBuilder::<WakeEvent>::with_user_event();
    // Tao invokes this supported hook before DispatchMessage, outside its user-handler borrow.
    // The modal loan leaves nested callbacks with fixed deferred facts only.
    builder.with_msg_hook(move |_| {
        native_owner.message_turn();
        false
    });
    let mut event_loop = builder.build();
    let proxy = event_loop.create_proxy();
    let lifetime = crate::completion_wake::WakeLifetime::new();
    let wake = Arc::new(crate::completion_wake::ProxyWake::new(
        proxy.clone(),
        lifetime.clone(),
        WakeEvent::Wake,
    ));
    {
        let mut loan =
            owner.loan().ok_or_else(|| ReportSurfaceError::rejected("report_initial_loan"))?;
        let state = loan.state()?;
        let initialization = state.content.start(wake).and_then(|()| {
            state.apartment = Some(crate::windows_ui_apartment::UiApartment::enter()?);
            let window = WindowBuilder::new()
                .with_title(title)
                .with_visible(false)
                .with_theme(Some(if dark { Theme::Dark } else { Theme::Light }))
                .with_inner_size(LogicalSize::new(width, height))
                .with_min_inner_size(LogicalSize::new(360, 280))
                .build(&event_loop)
                .map_err(|cause| ReportSurfaceError::new("report_parent_create", cause))?;
            state.session.initialize(Rc::new(window), proxy.clone(), lease, &mut state.content)
        });
        owner.deferred.bind(state.session.window_id);
        if let Err(cause) = initialization {
            state.session.fail(&mut state.content, cause);
        }
    }
    let exit_status = event_loop.run_return(|event, _, flow| {
        *flow = crate::cadence::closing_wait(Instant::now());
        let Some(mut loan) = owner.loan() else {
            if let Event::WindowEvent { window_id, event, .. } = &event {
                owner.deferred.capture(*window_id, event);
            }
            *flow = ControlFlow::Wait;
            return;
        };
        let state = match loan.state() {
            Ok(state) => state,
            Err(cause) => {
                owner.deferred.fail(cause);
                *flow = ControlFlow::Exit;
                return;
            }
        };
        owner.apply_deferred(state);
        match event {
            Event::WindowEvent { window_id, event, .. }
                if Some(window_id) == state.session.window_id =>
            {
                match event {
                    WindowEvent::Destroyed => state.session.destroyed = true,
                    WindowEvent::CloseRequested => {
                        let input = Input {
                            binding: state.session.state.binding(),
                            intent: super::state::Intent::Action(ReportAction::Dismiss),
                        };
                        if let Err(cause) = state.session.input(&mut state.content, input) {
                            state.session.fail(&mut state.content, cause);
                        }
                    }
                    WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                        if let Err(cause) =
                            state.session.controls.as_mut().map_or(Ok(()), |c| c.layout())
                        {
                            state.session.fail(&mut state.content, cause);
                        }
                    }
                    _ => {}
                }
            }
            Event::MainEventsCleared => {
                if state.session.state.closing() {
                    state.session.shutdown(&mut state.content);
                    state.session.observe_retirement(&mut state.content);
                } else {
                    let mut remaining = 32;
                    let inputs = state.session.controls.as_ref().map(|c| c.callback.take());
                    if let Some(inputs) = inputs {
                        for input in inputs.into_iter().flatten() {
                            remaining -= 1;
                            if let Err(cause) = state.session.input(&mut state.content, input) {
                                state.session.fail(&mut state.content, cause);
                                break;
                            }
                        }
                    }
                    let failure =
                        state.session.controls.as_ref().and_then(|c| c.callback.take_failure());
                    if let Some(cause) = failure {
                        state.session.fail(&mut state.content, cause);
                    }
                    if state.session.prepared_save.is_some() {
                        if !state.modal_wake_queued {
                            state.modal_wake_queued = true;
                            if let Err(cause) = proxy.send_event(WakeEvent::Wake) {
                                state.session.fail(
                                    &mut state.content,
                                    ReportSurfaceError::new("report_picker_dispatch_wake", cause),
                                );
                            }
                        }
                    } else if let Some(maximum) = std::num::NonZeroUsize::new(remaining)
                        && let Err(cause) = state.session.poll(&mut state.content, maximum)
                    {
                        state.session.fail(&mut state.content, cause);
                    }
                }
                if state.session.teardown_ready() {
                    *flow = ControlFlow::Exit;
                } else if state.session.close_deadline.is_some_and(|d| Instant::now() >= d) {
                    state.expired = true;
                    *flow = ControlFlow::Exit;
                }
            }
            _ => {}
        }
    });
    lifetime.disconnect();
    {
        let mut loan =
            owner.loan().ok_or_else(|| ReportSurfaceError::rejected("report_final_loan"))?;
        let state = loan.state()?;
        if !state.session.teardown_ready() {
            if !state.expired {
                state.session.fail(
                    &mut state.content,
                    ReportSurfaceError::new(
                        "report_native_loop_return",
                        std::io::Error::other(format!(
                            "Native loop returned with status {exit_status} before checked teardown"
                        )),
                    ),
                );
            }
            let cause =
                state.session.failure.take().unwrap_or_else(|| {
                    ReportSurfaceError::rejected("report_native_shutdown_deadline")
                });
            drop(loan);
            // Keep the one actual box, its claim, apartment and all unresolved native/content owners.
            std::mem::forget(event_loop);
            std::mem::forget(owner);
            return Err(cause);
        }
    }
    drop(event_loop);
    let mut state = owner.take_final()?;
    if exit_status != 0 {
        state.session.retain(ReportSurfaceError::new(
            "report_native_loop_status",
            std::io::Error::other(format!("Native loop returned with status {exit_status}")),
        ));
    }
    let mut final_failure = None;
    owner.finish_content(
        state.content,
        || {
            if let Some(apartment) = &mut state.apartment {
                // SAFETY: Actual native holders/loop retired; content finishes and drops under its UI STA.
                unsafe { apartment.release_after_teardown() }
            } else {
                Ok(())
            }
        },
        &mut final_failure,
    );
    if let Some(cause) = final_failure {
        state.session.retain(cause.into());
    }
    if let Some(cause) = state.session.failure {
        return Err(owner.fund_failure(cause));
    }
    state.session.state.receipt()
}

/// Refuse oversized authored metadata before any bootstrap/start/native effects.
pub(super) fn validate_content_capacity<T>() -> Result<(), ReportSurfaceError> {
    if dispatch::storage_bytes::<T>() as u64 > ReportSurfaceOptions::DISPATCH_STORAGE_BYTES {
        return Err(ReportSurfaceError::rejected("report_dispatch_capacity"));
    }
    Ok(())
}
