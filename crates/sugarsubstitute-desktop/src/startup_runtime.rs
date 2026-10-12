// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Pump splash, nonblocking browser construction and composing execution on one outer STA.

#[path = "startup_custody.rs"]
mod custody;
#[path = "startup_events.rs"]
mod events;
#[path = "native_activity.rs"]
mod native_activity;
#[path = "startup_surface.rs"]
mod surface;
#[path = "windows_activity.rs"]
mod windows_activity;
use crate::{
    DesktopError, DesktopOptions, DesktopStartupObservation, StartupDesktopReceipt,
    StartupWindowContent, main_actor::MainActor, native_action::NativeAction,
};
use std::{num::NonZeroUsize, sync::Arc, time::Instant};
use tao::{
    event_loop::{ControlFlow, EventLoopBuilder},
    platform::run_return::EventLoopExtRunReturn,
};

use crate::timing::{NativeStartupStage, observe_startup};

struct Session {
    activity: native_activity::NativeActivity,
    observers: windows_activity::Observers,
    splash: surface::Splash,
    main: Option<MainActor>,
    closing: bool,
    failure: Option<DesktopError>,
    pointer: crate::Pointer,
    pressed: Option<crate::HitTarget>,
    next_frame: Instant,
    started: Instant,
    redraw: bool,
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
        content.observe(observation).map_err(|cause| DesktopError::boxed("observe_startup", cause))
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
    let timings = crate::timing::TimingObserver::native(options.audit_timings)?;
    let mut apartment = crate::windows_ui_apartment::UiApartment::enter()?;
    let mut event_loop = EventLoopBuilder::<NativeAction>::with_user_event().build();
    observe_startup(timings.as_ref(), NativeStartupStage::EventLoopReady)?;
    let splash =
        match surface::Splash::new(&options, &event_loop, &content, started, timings.clone()) {
            Ok(splash) => splash,
            Err(cause) => {
                std::mem::forget(event_loop);
                std::mem::forget(apartment);
                return Err(cause);
            }
        };
    let wake_lifetime = crate::completion_wake::WakeLifetime::new();
    let wake = Arc::new(crate::completion_wake::ProxyWake::new(
        event_loop.create_proxy(),
        wake_lifetime.clone(),
        NativeAction::Completions,
    ));
    let drain = NonZeroUsize::new(32)
        .ok_or_else(|| DesktopError::message("startup_drain", "zero budget"))?;
    let mut session = Session {
        activity: native_activity::NativeActivity::new(content.run_key()),
        observers: windows_activity::Observers::new(event_loop.create_proxy()),
        splash,
        main: None,
        closing: false,
        failure: None,
        pointer: crate::Pointer::default(),
        pressed: None,
        next_frame: started,
        started,
        redraw: false,
        drain,
        splash_retirement_reported: false,
        main_failures_reported: false,
        navigations: 0,
        main_material: None,
    };
    if let Err(cause) = content.start(wake) {
        session.fail(&mut content, DesktopError::boxed("start_startup_content", cause));
    } else {
        let shown = (|| {
            observe_startup(timings.as_ref(), NativeStartupStage::ExecutionReady)?;
            session.splash.window()?.set_visible(true);
            session.splash.window()?.set_focus();
            observe_startup(timings.as_ref(), NativeStartupStage::ShowFocusReturned)
        })();
        if let Err(cause) = shown {
            session.fail(&mut content, cause);
        }
    }
    let mut animation_period = crate::windows_animation_period::WindowsAnimationPeriod::new();
    let proxy = event_loop.create_proxy();
    let context = events::Context { title: &options.title, timings: timings.as_ref(), limit };
    let mut custody = custody::LoopCustody::new();
    let teardown = loop {
        let exit_code = event_loop.run_return(|event, target, control| {
            let began = timings.as_ref().map(crate::timing::TimingObserver::mark).transpose();
            let kind = crate::native_surface::callback_kind(&event);
            let result = match began {
                Ok(began) => crate::timing::observe_callback(timings.as_ref(), began, kind, || {
                    events::dispatch(
                        &mut session,
                        &mut content,
                        event,
                        target,
                        proxy.clone(),
                        &context,
                    )
                }),
                Err(cause) => Err(cause),
            };
            if let Err(cause) = result {
                session.fail(&mut content, cause);
            }
            if let Some(cause) =
                timings.as_ref().and_then(crate::timing::TimingObserver::take_failure)
            {
                session.fail(&mut content, cause);
            }
            if let Err(cause) = animation_period.reconcile(
                !session.closing
                    && !session.splash.retiring
                    && session.splash.state.visible
                    && content.animating(),
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
    let (splash_receipt, failure, main_material, navigations_started) = teardown.finish(|| {
        if let Err(cause) = animation_period.shutdown() {
            session.retain(cause);
        }
        drop(animation_period);
        wake_lifetime.disconnect();
        if let Err(cause) = content.finish_shutdown() {
            session.retain(DesktopError::boxed("finish_startup_shutdown", cause));
        }
        if let Some(main) = session.main.take() {
            session.navigations += main.navigations();
            session.main_material = main.material();
        }
        let splash_receipt = session.splash.receipt.take();
        let Session { splash, mut failure, main_material, navigations, .. } = session;
        drop(splash);
        drop(event_loop);
        // SAFETY: The consumed permit proves actual native retirement and owner settlement;
        // all retired native holders and the same event loop have now been dropped on this STA.
        if let Err(cause) = unsafe { apartment.release_after_teardown() } {
            DesktopError::retain_first(&mut failure, cause);
        }
        (splash_receipt, failure, main_material, navigations)
    });
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
