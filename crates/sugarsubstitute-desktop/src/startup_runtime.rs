// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Pump splash, nonblocking browser construction and composing execution on one outer STA.

use crate::shutdown as custody;
#[path = "startup_admission.rs"]
mod admission;
#[path = "startup_events.rs"]
mod events;
#[path = "startup_main_fence.rs"]
mod main_fence;
#[path = "native_activity.rs"]
mod native_activity;
#[path = "startup_preparation.rs"]
mod preparation;
#[path = "startup_surface.rs"]
mod surface;
#[cfg(test)]
#[path = "startup_preparation_test_content.rs"]
mod test_content;
#[path = "windows_activity.rs"]
mod windows_activity;
use crate::{
    DesktopError, DesktopOptions, DesktopStartupObservation, StartupDesktopReceipt,
    StartupWindowContent, main_actor::MainActor, native_action::NativeAction,
};
use std::{num::NonZeroUsize, time::Instant};
use tao::{event_loop::ControlFlow, platform::run_return::EventLoopExtRunReturn};

struct Session {
    activity: native_activity::NativeActivity,
    accent: crate::windows_system_accent::SystemAccent,
    observers: windows_activity::Observers,
    /// Answers Windows shutdown and logoff from the close save; absent if Tao changed.
    session_end: Option<crate::windows_session_end::SessionEndWatch>,
    splash: surface::Splash,
    main: Option<MainActor>,
    main_fence: main_fence::MainFence,
    closing: bool,
    failure: Option<DesktopError>,
    pointer: crate::Pointer,
    pressed: Option<crate::HitTarget>,
    next_frame: Instant,
    started: Instant,
    drain: NonZeroUsize,
    splash_retirement_reported: bool,
    main_failures_reported: bool,
    navigations: u64,
    main_material: Option<crate::NativeMaterial>,
}

impl Session {
    fn shutdown_facts(&self) -> custody::ShutdownFacts {
        custody::ShutdownFacts {
            closing: self.closing,
            splash_retired: self.splash.retired,
            main_retired: self.main.as_ref().is_none_or(MainActor::retired)
                && self.observers.retired()
                && self.activity.retired(),
            execution_settled: self.splash.state.settled,
        }
    }
    fn close(&mut self, content: &mut impl StartupWindowContent) {
        if self.closing {
            return;
        }
        self.closing = true;
        self.activity.close();
        self.observers.close();
        self.splash.state.close(content);
        if let Err(cause) = self.splash.begin_retirement(content) {
            self.retain(cause);
        }
        if let Some(main) = &mut self.main {
            let result = main.close(main.identity());
            if let Err(cause) = result {
                self.retain(cause);
            }
        }
    }
    fn retain(&mut self, cause: DesktopError) {
        DesktopError::retain_first(&mut self.failure, cause);
    }
    fn fail(&mut self, content: &mut impl StartupWindowContent, cause: DesktopError) {
        self.retain(cause);
        self.close(content);
    }
    fn observe(
        &mut self,
        content: &mut impl StartupWindowContent,
        observation: DesktopStartupObservation,
    ) -> Result<(), DesktopError> {
        let kind = DesktopError::startup_observation_kind(&observation);
        content.observe(observation).map_err(|cause| DesktopError::observed_startup(kind, cause))
    }
}

/// Run real native startup without pumping a second loop or waiting for a creation callback.
///
/// The caller supplies application startup policy, backend/profile authority and execution.
/// Every callback only records bounded facts and wakes this loop. Actual owner shutdown,
/// native registration retirement and STA release precede a successful returned receipt.
///
/// # Errors
/// Preserve original owner/native causes. Partial native resources remain retained on teardown failure.
pub fn run_startup(
    options: DesktopOptions,
    mut content: impl StartupWindowContent,
) -> Result<StartupDesktopReceipt, DesktopError> {
    if options.logical_width == 0 || options.logical_height == 0 {
        return Err(DesktopError::message(
            "create_startup_splash",
            "client dimensions must be positive",
        ));
    }
    let started = Instant::now();
    let limit = options
        .duration
        .map(|duration| {
            started.checked_add(duration).ok_or_else(|| {
                DesktopError::message(
                    "startup_qualification_duration",
                    "native duration out of range",
                )
            })
        })
        .transpose()?;
    let timings = crate::timing::TimingObserver::native(
        options.audit_timings,
        crate::timing::NativeStartupOrder::ExecutionFirst,
    )?;
    let bootstrap = crate::bootstrap::Bootstrap::new()?;
    let drain = NonZeroUsize::MIN.saturating_add(31);
    let (mut event_loop, wake_lifetime) =
        match admission::admit(&bootstrap, &mut content, drain, limit, timings.as_ref()) {
            Ok(admitted) => admitted,
            Err(cause) => {
                let mut failure = Some(cause);
                finish_before_loop(content, &mut failure);
                if let Err(cause) = timings.map(crate::timing::TimingObserver::finish).transpose() {
                    DesktopError::retain_first(&mut failure, cause);
                }
                return Err(failure.unwrap_or_else(|| {
                    DesktopError::message("startup_preparation", "Tao admission failed")
                }));
            }
        };
    let mut apartment = match crate::windows_ui_apartment::UiApartment::enter() {
        Ok(apartment) => apartment,
        Err(cause) => {
            let mut failure = preparation::close_before_surface(
                &mut event_loop,
                &mut content,
                drain,
                limit,
                cause,
            );
            bootstrap.retain_close_failure(&mut failure);
            wake_lifetime.disconnect();
            drop(event_loop);
            custody::finish_after_native(
                content,
                |content| {
                    content
                        .finish_shutdown()
                        .map_err(|cause| DesktopError::boxed("finish_preparation_shutdown", cause))
                },
                || Ok(()),
                &mut failure,
            );
            if let Err(cause) = timings.map(crate::timing::TimingObserver::finish).transpose() {
                DesktopError::retain_first(&mut failure, cause);
            }
            return Err(failure.unwrap_or_else(|| {
                DesktopError::message("startup_preparation", "STA admission failed")
            }));
        }
    };
    let splash = surface::Splash::new(&options, &content, started, timings.clone());
    let mut session = Session {
        activity: native_activity::NativeActivity::new(content.run_key()),
        accent: crate::windows_system_accent::SystemAccent::observe(
            event_loop.create_proxy(),
            wake_lifetime.clone(),
        ),
        observers: windows_activity::Observers::new(event_loop.create_proxy()),
        session_end: session_end_watch(&content, event_loop.create_proxy()),
        splash,
        main: None,
        main_fence: main_fence::MainFence::new(),
        closing: false,
        failure: None,
        pointer: crate::Pointer::default(),
        pressed: None,
        next_frame: started,
        started,
        drain,
        splash_retirement_reported: false,
        main_failures_reported: false,
        navigations: 0,
        main_material: None,
    };
    // The first splash frame and the main window are prepared with the current accent.
    if let Err(cause) = events::system_accent(&mut session, &mut content) {
        session.fail(&mut content, cause);
    }
    if let Err(cause) = session.splash.initialize(&options, &event_loop, &content, timings.clone())
    {
        session.fail(&mut content, cause);
    }
    let mut animation_period = crate::windows_animation_period::WindowsAnimationPeriod::new();
    let proxy = event_loop.create_proxy();
    let context = events::Context { title: &options.title, timings: timings.as_ref(), limit };
    let mut custody = custody::LoopCustody::new();
    let teardown = loop {
        let exit_code = event_loop.run_return(|event, target, control| {
            let kind = crate::native_surface::callback_kind(&event);
            let result = observed_dispatch(timings.as_ref(), session.closing, kind, || {
                events::dispatch(&mut session, &mut content, event, target, proxy.clone(), &context)
            });
            if let Err(cause) = result {
                session.fail(&mut content, cause);
            }
            if let Some(watch) = &mut session.session_end {
                watch.set_outstanding(!content.close_save_settled());
                if let Err(cause) = watch.track() {
                    session.retain(cause);
                }
            }
            if let Some(cause) =
                timings.as_ref().and_then(crate::timing::TimingObserver::take_failure)
            {
                session.fail(&mut content, cause);
            }
            // Display-rate canvas deadlines are shorter than the default system timer period.
            if let Err(cause) = animation_period.reconcile(
                !session.closing
                    && ((!session.splash.retiring
                        && session.splash.state.visible
                        && content.animating())
                        || content.main_canvas_animating()),
            ) {
                session.fail(&mut content, cause);
            }
            *control = if custody.may_exit(session.shutdown_facts()) {
                ControlFlow::Exit
            } else {
                events::wait(&session, &content, limit)
            };
        });
        let returned = custody.native_return(session.shutdown_facts(), exit_code);
        if let Some(cause) = returned.failure {
            session.fail(&mut content, cause);
        }
        if let Some(teardown) = returned.teardown {
            break teardown;
        }
        // The same loop resumes retained native work and the original owner shutdown budget.
    };
    let (splash_receipt, failure, main_material, navigations_started) = teardown.finish(
        || {
            if let Err(cause) = animation_period.shutdown() {
                session.retain(cause);
            }
            drop(animation_period);
            bootstrap.retain_close_failure(&mut session.failure);
            session.accent.retire();
            if let Some(watch) = session.session_end.take()
                && let Err(cause) = watch.retire()
            {
                session.retain(cause);
            }
            wake_lifetime.disconnect();
            if let Some(main) = session.main.take() {
                session.navigations += main.navigations();
                session.main_material = main.material();
            }
            let splash_receipt = session.splash.receipt.take();
            let Session { splash, failure, main_material, navigations, .. } = session;
            drop(splash);
            drop(event_loop);
            (splash_receipt, failure, main_material, navigations)
        },
        |(splash_receipt, mut failure, main_material, navigations)| {
            custody::finish_after_native(
                content,
                |content| {
                    content
                        .finish_shutdown()
                        .map_err(|cause| DesktopError::boxed("finish_startup_shutdown", cause))
                },
                || {
                    // SAFETY: Actual native holders/loop retired; the actual content finalizer
                    // succeeded and its owner dropped while this same STA reference stayed held.
                    unsafe { apartment.release_after_teardown() }
                },
                &mut failure,
            );
            (splash_receipt, failure, main_material, navigations)
        },
    );
    let timing_receipt = timings.map(crate::timing::TimingObserver::finish).transpose();
    if let Some(cause) = failure {
        return Err(cause);
    }
    let mut splash = splash_receipt.ok_or_else(|| {
        DesktopError::message("startup_splash_receipt", "retirement receipt missing")
    })?;
    splash.timings = timing_receipt?;
    Ok(StartupDesktopReceipt { splash, main_material, navigations_started })
}

/// Answer session ends from the content's close save. Without the watch Tao would exit the
/// process at session end, so its absence is logged as an error rather than ending startup.
fn session_end_watch(
    content: &impl StartupWindowContent,
    proxy: tao::event_loop::EventLoopProxy<NativeAction>,
) -> Option<crate::windows_session_end::SessionEndWatch> {
    let wake = move || {
        if let Err(cause) = proxy.send_event(NativeAction::SessionEnding) {
            tracing::error!(operation = "session_end_wake", cause = %cause,
                "A session end could not wake the event loop");
        }
    };
    match crate::windows_session_end::SessionEndWatch::install(content.session_end_reason(), wake) {
        Ok(watch) => Some(watch),
        Err(cause) => {
            tracing::error!(operation = "install_session_end", cause = %cause,
                "Windows shutdown and logoff will not wait for the close save");
            None
        }
    }
}

/// No Tao/window was admitted; actual settled content finalization still owns worker joins.
fn finish_before_loop(content: impl StartupWindowContent, failure: &mut Option<DesktopError>) {
    custody::finish_after_native(
        content,
        |content| {
            content
                .finish_shutdown()
                .map_err(|cause| DesktopError::boxed("finish_preparation_shutdown", cause))
        },
        || Ok(()),
        failure,
    );
}

/// Failed optional timing cannot suppress actual closing callbacks or their original causes.
pub(crate) fn observed_dispatch(
    timings: Option<&crate::timing::TimingObserver>,
    closing: bool,
    kind: crate::timing::NativeCallbackKind,
    body: impl FnOnce() -> Result<(), DesktopError>,
) -> Result<(), DesktopError> {
    if closing {
        return body();
    }
    let began = timings.map(crate::timing::TimingObserver::mark).transpose()?;
    let result = body();
    let measured = match (timings, began) {
        (Some(observer), Some(begin)) => observer.callback(begin, kind, None),
        _ => Ok(()),
    };
    match result {
        Err(cause) => {
            if let Err(secondary) = measured {
                tracing::error!(operation="startup_dispatch_timing", cause=%secondary,
                    "Additional timing failure after native dispatch failed");
            }
            Err(cause)
        }
        Ok(()) => measured,
    }
}
