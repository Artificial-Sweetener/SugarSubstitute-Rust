// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit the actual Tao loop only after pre-Tao preparation, retaining failed partial custody.
use super::preparation::{self, PreparationExit};
use crate::{
    DesktopError, StartupWindowContent,
    bootstrap::{Bootstrap, construct_loop},
    completion_wake::{ProxyWake, WakeLifetime},
    native_action::NativeAction,
    timing::{NativeStartupStage, TimingObserver, observe_startup},
};
use std::{num::NonZeroUsize, sync::Arc, time::Instant};
use tao::event_loop::{EventLoop, EventLoopBuilder};

/// Start once, observe actual preparation, construct Tao and retarget the retained wake.
/// Failure returns only after the original content owner has actually settled;
/// checked event-close failures retain native custody in the returned error.
pub(super) fn admit(
    bootstrap: &Bootstrap,
    content: &mut impl StartupWindowContent,
    maximum: NonZeroUsize,
    limit: Option<Instant>,
    timings: Option<&TimingObserver>,
) -> Result<(EventLoop<NativeAction>, WakeLifetime), DesktopError> {
    admit_with(bootstrap, content, maximum, limit, timings, || {
        let event_loop =
            construct_loop(|| EventLoopBuilder::<NativeAction>::with_user_event().build())?;
        let lifetime = WakeLifetime::new();
        let target = Arc::new(ProxyWake::new(
            event_loop.create_proxy(),
            lifetime.clone(),
            NativeAction::Completions,
        ));
        Ok((
            event_loop,
            lifetime,
            target as Arc<dyn sugarsubstitute_contracts::execution::CompletionWake>,
        ))
    })
}
/// Isolate fallible native construction, leaving the preparation/handoff transaction intact.
fn admit_with<T>(
    bootstrap: &Bootstrap,
    content: &mut impl StartupWindowContent,
    maximum: NonZeroUsize,
    limit: Option<Instant>,
    timings: Option<&TimingObserver>,
    construct: impl FnOnce() -> Result<
        (T, WakeLifetime, Arc<dyn sugarsubstitute_contracts::execution::CompletionWake>),
        DesktopError,
    >,
) -> Result<(T, WakeLifetime), DesktopError> {
    let (prepared, mut failure) =
        preparation::prepare(bootstrap, content, bootstrap.wake(), maximum, limit, timings);
    if prepared == PreparationExit::Closed {
        retain_close(bootstrap, &mut failure);
        return Err(failure.unwrap_or_else(|| {
            DesktopError::message("startup_preparation", "Startup closed before Tao admission")
        }));
    }
    let (event_loop, lifetime, target) = match construct() {
        Ok(native) => native,
        Err(cause) => {
            failure = preparation::close_before_loop(bootstrap, content, maximum, limit, cause);
            retain_close(bootstrap, &mut failure);
            return Err(failure.unwrap_or_else(|| {
                DesktopError::message("startup_preparation", "Tao construction failed")
            }));
        }
    };
    let admitted = observe_startup(timings, NativeStartupStage::EventLoopReady)
        .and_then(|()| bootstrap.handoff(target));
    if let Err(cause) = admitted {
        failure = preparation::close_before_loop(bootstrap, content, maximum, limit, cause);
        retain_close(bootstrap, &mut failure);
        lifetime.disconnect();
        drop(event_loop);
        return Err(failure.unwrap_or_else(|| {
            DesktopError::message("startup_preparation", "Tao wake admission failed")
        }));
    }
    Ok((event_loop, lifetime))
}
fn retain_close(bootstrap: &Bootstrap, failure: &mut Option<DesktopError>) {
    bootstrap.retain_close_failure(failure);
}

#[cfg(test)]
#[path = "startup_admission_tests.rs"]
mod tests;
