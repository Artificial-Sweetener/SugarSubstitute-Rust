// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Send original authenticated completion only after actual native retirement and source return.
use super::*;
#[path = "retirement/control.rs"]
mod control;
#[path = "retirement/native.rs"]
mod native;
use std::{num::NonZeroU64, time::Duration};
use sugarsubstitute_execution::{
    DesktopCaptureTemporaryState, windows_capture_attach::CaptureCleanupSnapshot,
};
use sugarsubstitute_supervision::lifecycle_protocol::{
    DecodedPayload, FrameCodec, ReplyOutcome, WireMessage, WirePayload,
};
#[derive(Clone, Copy)]
enum Control {
    Activity { revision: NonZeroU64, activity: UiWatchdogActivity },
    MainRetired(MainIdentity),
    Release(LeaseReference),
    ExitIntent,
    ExitCompleted,
}
#[derive(Clone, Copy)]
struct Pending {
    header: WireHeader,
    original: Moment,
    control: Control,
    admitted: bool,
}
pub(super) struct ChildShutdown {
    budget: Option<ShutdownBudget>,
    closing: bool,
    native_retired: bool,
    pub(in super::super) main_retired: bool,
    pub(in super::super) main_retirement_ack: bool,
    intent_done: bool,
    completed_done: bool,
    release_requested: bool,
    released: bool,
    bootstrap_failure_observed: bool,
    shutdown: Option<ShutdownId>,
    pending: Option<Pending>,
    queued: [Option<Control>; 4],
}
impl ChildShutdown {
    pub(super) fn new() -> Self {
        Self {
            budget: None,
            closing: false,
            native_retired: false,
            main_retired: false,
            main_retirement_ack: false,
            intent_done: false,
            completed_done: false,
            release_requested: false,
            released: false,
            bootstrap_failure_observed: false,
            shutdown: None,
            pending: None,
            queued: [None; 4],
        }
    }
    pub(super) fn closing(&self) -> bool {
        self.closing
    }
    pub(super) fn deadline(&self) -> Option<Moment> {
        self.pending.map(|p| p.original).or(self.budget.map(|b| b.settle_until))
    }
    fn retain_budget(&mut self, budget: ShutdownBudget) {
        if self.budget.is_none_or(|old| budget.settle_until < old.settle_until) {
            self.budget = Some(budget)
        }
    }
    fn queue(&mut self, control: Control) -> Result<()> {
        let slot = self
            .queued
            .iter_mut()
            .find(|p| p.is_none())
            .ok_or("Bounded native lifecycle messages full")?;
        *slot = Some(control);
        Ok(())
    }
}
impl ProductionChild {
    /// Latch capture cleanup first; absent capture permits the composing content's original clock.
    /// # Errors
    /// Keeps clock/counter failures and actual partial capture/source custody unchanged.
    pub(in crate::production) fn begin_shutdown(&mut self) -> Result<Option<ShutdownBudget>> {
        self.observe_bootstrap_failure();
        if !self.shutdown.closing {
            self.shutdown.closing = true;
            self.bootstrap.close_initialization();
            self.main.close();
            self.output.close();
        }
        match self.bootstrap.capture_cleanup_snapshot() {
            Some(CaptureCleanupSnapshot::Latched { budget, counter }) => {
                self.shutdown.retain_budget(budget);
                if let Err(cause) = counter {
                    return Err(Box::new(cause));
                }
                Ok(self.shutdown.budget)
            }
            Some(CaptureCleanupSnapshot::ClockUnavailable(cause)) => Err(Box::new(cause)),
            Some(CaptureCleanupSnapshot::Requested) => {
                Err("Actual capture cleanup clock observation pending".into())
            }
            _ => Ok(self.shutdown.budget),
        }
    }
    /// Borrow the immutable actual transaction after independent capture reconciliation.
    pub(in crate::production) fn shutdown_budget(&self) -> Option<ShutdownBudget> {
        self.shutdown.budget.or(self.bootstrap.shutdown_budget())
    }
    /// Keep one original budget for all authenticated exit and native cleanup messages.
    /// # Errors
    /// Rejects mismatched repeated budgets without a new deadline or scope.
    pub(in crate::production) fn request_shutdown(&mut self, budget: ShutdownBudget) -> Result<()> {
        if ShutdownBudget::new(budget.started)? != budget {
            return Err("Shutdown budget is not the original ten/fifteen transaction".into());
        }
        self.shutdown.retain_budget(budget);
        if !self.bootstrap.channel_mut().admitted() {
            return Ok(());
        }
        if self.shutdown.shutdown.is_none() {
            self.shutdown.shutdown = Some(ShutdownId::new(1)?);
            self.shutdown.queue(Control::ExitIntent)?;
        }
        if !self.bootstrap.channel_mut().admitted() {
            return Ok(());
        }
        self.advance_shutdown()
    }
    fn observe_bootstrap_failure(&mut self) {
        if !self.shutdown.bootstrap_failure_observed
            && let Some(cause) = self.bootstrap.failure_source()
        {
            tracing::error!(operation = "child_shutdown", run = ?self.run, cause = %cause,
                "Retained original child bootstrap failure");
            self.shutdown.bootstrap_failure_observed = true;
        }
    }
    pub(super) fn advance_shutdown(&mut self) -> Result<()> {
        self.observe_bootstrap_failure();
        if self.shutdown.native_retired
            && !self.bootstrap.projection_pending()
            && !self.main.pending()
            && !self.output.pending()
        {
            self.bootstrap.release_source_after_native_retired()?;
            self.output.release_frame()?;
            self.main.release_frame()?;
            self.failed_frame = None;
            self.grant = None;
            if !self.shutdown.release_requested
                && let Some(lease) = self.lease
            {
                self.shutdown.queue(Control::Release(lease))?;
                self.shutdown.release_requested = true;
            }
            if self.shutdown.intent_done
                && (self.lease.is_none() || self.shutdown.released)
                && !self.shutdown.completed_done
                && !self
                    .shutdown
                    .pending
                    .is_some_and(|p| matches!(p.control, Control::ExitCompleted))
                && !self
                    .shutdown
                    .queued
                    .iter()
                    .flatten()
                    .any(|p| matches!(p, Control::ExitCompleted))
            {
                self.shutdown.queue(Control::ExitCompleted)?;
            }
        }
        self.advance_control()?;
        if self.shutdown.completed_done {
            self.bootstrap.channel_mut().request_close();
        }
        if self.permanent.is_none()
            && let Some(permanent) = self.bootstrap.try_take_process_lifetime()
        {
            self.permanent = Some(permanent?);
        }
        Ok(())
    }
    /// Observe actual worker completion, permanent capture transfer and channel checked retirement.
    /// # Errors
    /// Unmeasured or unsettled work remains owned; no deadline expiry fabricates completion.
    pub(in crate::production) fn poll_shutdown(&mut self) -> Result<bool> {
        if !self.shutdown.closing {
            return Ok(false);
        }
        self.activity(None)?;
        let finite =
            !self.bootstrap.projection_pending() && !self.main.pending() && !self.output.pending();
        let capture = match self.bootstrap.temporary_settled() {
            None => !self.bootstrap.capture_admitted(),
            Some(
                DesktopCaptureTemporaryState::UnissuedReturned
                | DesktopCaptureTemporaryState::Retired,
            ) => true,
            Some(DesktopCaptureTemporaryState::Transferred) => self.permanent.is_some(),
            Some(DesktopCaptureTemporaryState::Pending) => false,
        };
        let channel = !self.bootstrap.channel_mut().admitted()
            || self.bootstrap.channel_mut().terminal().is_some();
        Ok(finite && capture && channel && self.shutdown.native_retired)
    }
    /// Retain real scopes, typed terminals and permanent capsule through the caller's sole joins.
    /// # Errors
    /// Rejects finalization until actual dependent owners have settled.
    pub(in crate::production) fn finish_shutdown(&mut self) -> Result<()> {
        self.observe_bootstrap_failure();
        if self.finished {
            return Ok(());
        }
        if !self.poll_shutdown()? {
            return Err("Child real ownership remains unsettled".into());
        }
        self.finished = true;
        Ok(())
    }
}

#[cfg(test)]
#[path = "retirement/tests.rs"]
mod tests;
