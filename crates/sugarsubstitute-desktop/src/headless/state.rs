// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own the headless close/settlement decision independently of native callback delivery.
use super::{HeadlessLifetime, HeadlessStep};
use crate::{DesktopError, cadence::closing_wait};
use std::{num::NonZeroUsize, sync::Arc, time::Instant};
use sugarsubstitute_contracts::execution::CompletionWake;
use tao::event_loop::ControlFlow;

pub(super) struct HeadlessState {
    started: bool,
    prepared: bool,
    closing: bool,
    settled: bool,
    failure: Option<DesktopError>,
}
impl HeadlessState {
    pub(super) const fn new() -> Self {
        Self { started: false, prepared: false, closing: false, settled: false, failure: None }
    }
    pub(super) fn start(
        &mut self,
        lifetime: &mut impl HeadlessLifetime,
        wake: Arc<dyn CompletionWake>,
    ) {
        if self.started {
            self.fail(lifetime, DesktopError::message("headless_start", "Content already started"));
            return;
        }
        self.started = true;
        if let Err(cause) = lifetime.start(wake) {
            self.fail(lifetime, DesktopError::boxed("headless_start", cause));
        }
    }
    /// Only the exact role's actual preparation can admit Tao, never a wake/deadline.
    pub(super) fn prepare(&mut self, lifetime: &mut impl HeadlessLifetime) {
        if self.prepared || self.settled {
            return;
        }
        if !self.started {
            self.fail(lifetime, DesktopError::message("headless_prepare", "Content not started"));
        }
        if !self.closing {
            match lifetime.prepare_native_runtime(NonZeroUsize::MIN.saturating_add(31)) {
                Ok(super::StartupPreparationStep::Ready) => self.prepared = true,
                Ok(super::StartupPreparationStep::Closing) => self.close(lifetime),
                Ok(super::StartupPreparationStep::Waiting) => {}
                Err(cause) => self.fail(lifetime, DesktopError::boxed("headless_prepare", cause)),
            }
        }
        if self.closing {
            self.activity(lifetime);
            self.poll_shutdown(lifetime);
        }
    }
    pub(super) const fn prepared(&self) -> bool {
        self.prepared && !self.closing
    }
    pub(super) fn activity(&mut self, lifetime: &mut impl HeadlessLifetime) {
        if self.settled {
            return;
        }
        // A nonzero compile-time constant gives every callback the same fixed drain bound.
        match lifetime.activity(NonZeroUsize::MIN.saturating_add(31)) {
            Ok(HeadlessStep::Active) => {}
            Ok(HeadlessStep::Shutdown) => self.close(lifetime),
            Err(cause) => self.fail(lifetime, DesktopError::boxed("headless_activity", cause)),
        }
    }
    fn close(&mut self, lifetime: &mut impl HeadlessLifetime) {
        if self.closing {
            return;
        }
        self.closing = true;
        self.prepared = false;
        if let Err(cause) = lifetime.request_shutdown() {
            DesktopError::retain_first(
                &mut self.failure,
                DesktopError::boxed("headless_request_shutdown", cause),
            );
        }
    }
    pub(super) fn fail(&mut self, lifetime: &mut impl HeadlessLifetime, cause: DesktopError) {
        DesktopError::retain_first(&mut self.failure, cause);
        self.close(lifetime);
    }
    pub(super) fn poll_shutdown(&mut self, lifetime: &mut impl HeadlessLifetime) {
        if !self.closing || self.settled {
            return;
        }
        match lifetime.poll_shutdown() {
            Ok(settled) => self.settled = settled,
            Err(cause) => DesktopError::retain_first(
                &mut self.failure,
                DesktopError::boxed("headless_poll_shutdown", cause),
            ),
        }
    }
    pub(super) const fn settled(&self) -> bool {
        self.settled
    }
    pub(super) fn control(&self, lifetime: &impl HeadlessLifetime, now: Instant) -> ControlFlow {
        if self.settled {
            return ControlFlow::Exit;
        }
        let requested = lifetime.next_deadline();
        if self.closing {
            match (closing_wait(now), requested) {
                (ControlFlow::WaitUntil(closing), Some(requested)) => {
                    ControlFlow::WaitUntil(closing.min(requested))
                }
                (closing, _) => closing,
            }
        } else {
            requested.map_or(ControlFlow::Wait, ControlFlow::WaitUntil)
        }
    }
    pub(super) fn native_return(&mut self, lifetime: &mut impl HeadlessLifetime, exit_code: i32) {
        if !self.settled || exit_code != 0 {
            self.fail(
                lifetime,
                DesktopError::message(
                    "headless_native_loop",
                    format!(
                        "Native event loop returned before clean settlement (status {exit_code})"
                    ),
                ),
            );
        }
    }
    pub(super) fn retire_bootstrap(&mut self, bootstrap: &crate::bootstrap::Bootstrap) {
        bootstrap.retain_close_failure(&mut self.failure);
    }
    pub(super) fn finish(
        self,
        lifetime: &mut impl HeadlessLifetime,
        native_teardown: impl FnOnce(),
    ) -> Result<(), DesktopError> {
        let mut failure = self.failure;
        if !self.settled {
            DesktopError::retain_first(
                &mut failure,
                DesktopError::message(
                    "headless_finish",
                    "Actual owner settlement has not been observed",
                ),
            );
        } else {
            native_teardown();
            if let Err(cause) = lifetime.finish_shutdown() {
                DesktopError::retain_first(
                    &mut failure,
                    DesktopError::boxed("headless_finish_shutdown", cause),
                );
            }
        }
        match failure {
            Some(cause) => Err(cause),
            None => Ok(()),
        }
    }
}
#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
