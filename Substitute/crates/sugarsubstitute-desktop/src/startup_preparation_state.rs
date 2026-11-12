// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain one pre-surface preparation transaction through actual owner settlement.

use crate::{
    DesktopError, DesktopStartupObservation, StartupPreparationStep, StartupWindowContent,
};
use std::{num::NonZeroUsize, sync::Arc};
use sugarsubstitute_contracts::execution::CompletionWake;

/// Distinguish a deliberate prepared transition from actually settled early closure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::startup_runtime) enum PreparationExit {
    /// Keep the same wake and execution lifetime for loop and surface construction.
    Ready,
    /// No surface was created and the composing owner actually settled.
    Closed,
}

/// Close once, preserve the first cause and never substitute intent for settlement.
pub(super) struct PreparationState {
    started: bool,
    ready: bool,
    closing: bool,
    settled: bool,
    failure: Option<DesktopError>,
}

impl PreparationState {
    /// Construct inert state before the single start attempt.
    pub(super) const fn new() -> Self {
        Self { started: false, ready: false, closing: false, settled: false, failure: None }
    }

    /// Mark the attempt before invoking the fallible external content owner.
    pub(super) fn start(
        &mut self,
        content: &mut impl StartupWindowContent,
        wake: Arc<dyn CompletionWake>,
    ) -> bool {
        if self.started {
            self.fail(
                content,
                DesktopError::message("start_preparation", "start already attempted"),
            );
            return false;
        }
        self.started = true;
        if let Err(cause) = content.start(wake) {
            self.fail(content, DesktopError::boxed("start_preparation", cause));
            return false;
        }
        true
    }

    /// Route one bounded owner turn, including while shutdown remains incomplete.
    pub(super) fn activity(
        &mut self,
        content: &mut impl StartupWindowContent,
        maximum: NonZeroUsize,
    ) {
        if self.ready || self.settled {
            return;
        }
        if !self.started {
            self.fail(
                content,
                DesktopError::message("prepare_native_ui", "content start was not attempted"),
            );
            return;
        }
        match content.prepare_native_ui(maximum) {
            Ok(StartupPreparationStep::Ready) if !self.closing => self.ready = true,
            Ok(StartupPreparationStep::Closing) => self.close(content),
            Ok(_) => {}
            Err(cause) => self.fail(content, DesktopError::boxed("prepare_native_ui", cause)),
        }
        if self.closing {
            match content.poll_shutdown() {
                Ok(settled) => self.settled = settled,
                Err(cause) => self.retain(DesktopError::boxed("poll_preparation_shutdown", cause)),
            }
        }
    }

    /// Actual absence of every application surface permits a native-retired observation.
    pub(super) fn close(&mut self, content: &mut impl StartupWindowContent) {
        if self.closing {
            return;
        }
        self.closing = true;
        self.ready = false;
        if let Err(cause) = content.request_shutdown() {
            self.retain(DesktopError::boxed("request_preparation_shutdown", cause));
        }
        let run = content.run_key();
        if let Err(cause) = content.observe(DesktopStartupObservation::NativeRetired(run)) {
            self.retain(DesktopError::boxed("retire_absent_startup_surfaces", cause));
        }
    }

    /// Preserve an unexpected original cause while entering the same close transaction.
    pub(super) fn fail(&mut self, content: &mut impl StartupWindowContent, cause: DesktopError) {
        self.retain(cause);
        self.close(content);
    }

    fn retain(&mut self, cause: DesktopError) {
        DesktopError::retain_first(&mut self.failure, cause);
    }

    /// Only actual preparation or actual settled early closure can leave this phase.
    pub(super) fn exit(&self) -> Option<PreparationExit> {
        if self.ready && !self.closing {
            Some(PreparationExit::Ready)
        } else if self.closing && self.settled {
            Some(PreparationExit::Closed)
        } else {
            None
        }
    }

    /// An early/nonzero native return closes admission without releasing retained work.
    pub(super) fn native_return(
        &mut self,
        content: &mut impl StartupWindowContent,
        status: i32,
    ) -> Option<PreparationExit> {
        if status != 0 || self.exit().is_none() {
            self.fail(content, DesktopError::message(
                "startup_preparation_loop",
                format!("Native preparation loop returned before its admitted transition (status {status})"),
            ));
        }
        self.exit()
    }

    /// Closing cadence remains bounded while native callbacks and workers retain custody.
    pub(super) const fn closing(&self) -> bool {
        self.closing
    }

    /// Resume existing started ownership after a fallible pre-surface native boundary.
    pub(super) fn closing_started(
        content: &mut impl StartupWindowContent,
        cause: DesktopError,
    ) -> Self {
        let mut state = Self { started: true, ..Self::new() };
        state.fail(content, cause);
        state
    }

    /// Transfer the first cause only after this owner established the exit disposition.
    pub(super) fn take_failure(&mut self) -> Option<DesktopError> {
        self.failure.take()
    }
}

#[cfg(test)]
#[path = "startup_preparation_state_tests.rs"]
mod tests;
