// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Send original authenticated completion only after actual native retirement and source return.
use super::*;
#[path = "retirement/control.rs"]
mod control;
#[path = "retirement/diagnostic.rs"]
mod diagnostic;
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
/// Preserve pending capture clock observation separately from an absent capture owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production) enum ShutdownInitiation {
    /// No capture session was admitted; the composing owner measures its own clock once.
    NoCapture,
    /// The existing capture claimant is still observing its original clock.
    CapturePending,
    /// The genuine capture owner supplied its unchanged ten/fifteen-second transaction.
    Captured(ShutdownBudget),
}
pub(super) struct ChildShutdown {
    budget: Option<ShutdownBudget>,
    closing: bool,
    native_retired: bool,
    pub(in super::super) main_retired: bool,
    pub(in super::super) main_retirement_ack: bool,
    pub(in super::super) main_absence_pending: bool,
    intent_done: bool,
    completed_done: bool,
    release_requested: bool,
    released: bool,
    bootstrap_failure_observed: bool,
    shutdown: Option<ShutdownId>,
    intent: Option<CompletionIntent>,
    pending: Option<Pending>,
    queued: [Option<Control>; 4],
}
impl ChildShutdown {
    /// Accept only a requested current native absence fact after actual application
    /// construction custody ended; this creates no native fact or settlement ACK.
    fn record_main_absence(
        &mut self,
        expected: MainIdentity,
        observed: MainIdentity,
        construction_pending: bool,
        closing_profile: bool,
    ) -> Result<()> {
        if expected != observed
            || construction_pending
            || self.main_retired
            || !(self.main_absence_pending || closing_profile)
        {
            return Err("Native Main absence is foreign, unrequested or still constructing".into());
        }
        self.queue(Control::MainRetired(observed))?;
        self.main_absence_pending = false;
        self.main_retired = true;
        Ok(())
    }
    pub(super) fn new() -> Self {
        Self {
            budget: None,
            closing: false,
            native_retired: false,
            main_retired: false,
            main_retirement_ack: false,
            main_absence_pending: false,
            intent_done: false,
            completed_done: false,
            release_requested: false,
            released: false,
            bootstrap_failure_observed: false,
            shutdown: None,
            intent: None,
            pending: None,
            queued: [None; 4],
        }
    }
    pub(super) fn closing(&self) -> bool {
        self.closing
    }
    pub(super) fn deadline(&self) -> Option<Moment> {
        [self.pending.map(|p| p.original), self.budget.map(|b| b.settle_until)]
            .into_iter()
            .flatten()
            .min()
    }
    fn retain_budget(&mut self, budget: ShutdownBudget) {
        if self.budget.is_none_or(|old| budget.settle_until < old.settle_until) {
            self.budget = Some(budget)
        }
    }
    /// Classify the actual capture owner's passive observation without sampling a clock.
    fn observe_capture(
        &mut self,
        snapshot: Option<CaptureCleanupSnapshot>,
    ) -> Result<ShutdownInitiation> {
        match snapshot {
            Some(CaptureCleanupSnapshot::Latched { budget, counter }) => {
                self.retain_budget(budget);
                if let Err(cause) = counter {
                    return Err(Box::new(cause));
                }
                Ok(ShutdownInitiation::Captured(
                    self.budget.ok_or("Actual capture shutdown budget absent")?,
                ))
            }
            Some(CaptureCleanupSnapshot::ClockUnavailable(cause)) => Err(Box::new(cause)),
            Some(CaptureCleanupSnapshot::Requested) => Ok(ShutdownInitiation::CapturePending),
            Some(CaptureCleanupSnapshot::Dormant) => {
                Err("Capture cleanup did not fence its original owner".into())
            }
            None => {
                Ok(self.budget.map_or(ShutdownInitiation::NoCapture, ShutdownInitiation::Captured))
            }
        }
    }
    /// Final completion still requires the genuine Child budget despite an early intent ACK.
    /// Native/source retirement remains a separate prerequisite at the composing callsite.
    fn completion_ready(&self, lease_required: bool) -> bool {
        self.budget.is_some()
            && self.intent_done
            && (!lease_required || self.released)
            && !self.completed_done
            && !self.pending.is_some_and(|p| matches!(p.control, Control::ExitCompleted))
            && !self.queued.iter().flatten().any(|p| matches!(p, Control::ExitCompleted))
    }
    /// Encode only the first whole-close classification, independently of later policy/failure phases.
    fn exit_intent_payload(&self) -> Result<WirePayload<'static>> {
        Ok(WirePayload::ExitIntent {
            shutdown: self.shutdown.ok_or("Actual shutdown identity absent")?,
            intent: self.intent.ok_or("Original whole-close intent absent")?,
            expected_code: 0,
        })
    }
    /// Queue deliberate whole-content exit independently of capture clock measurement.
    /// Neither this intent nor its eventual ACK supplies a Child shutdown budget or native proof.
    fn request_exit_intent(&mut self, registered: bool) -> Result<()> {
        if !registered {
            return Ok(());
        }
        if self.intent.is_none() {
            return Err("Original whole-close intent absent".into());
        }
        if self.shutdown.is_none() {
            let shutdown = ShutdownId::new(1)?;
            self.queue(Control::ExitIntent)?;
            self.shutdown = Some(shutdown);
        }
        Ok(())
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
    pub(in crate::production) fn begin_shutdown(&mut self) -> Result<ShutdownInitiation> {
        if self.shutdown.intent.is_none() {
            self.shutdown.intent = Some(self.completion_intent());
        }
        self.observe_bootstrap_failure();
        if !self.shutdown.closing {
            self.shutdown.closing = true;
            self.bootstrap.close_initialization();
            self.main.close();
            self.output.close();
        }
        // The existing control owner freezes its own header before capture measurement
        // completes. Genuine capture custody and its independent budget remain unchanged.
        let registered = self.bootstrap.channel_mut().registered();
        self.shutdown.request_exit_intent(registered)?;
        if registered {
            self.advance_control()?;
        }
        self.shutdown.observe_capture(self.bootstrap.capture_cleanup_snapshot())
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
        if self.shutdown.intent.is_none() {
            self.shutdown.intent = Some(self.completion_intent());
        }
        self.shutdown.retain_budget(budget);
        if !self.bootstrap.channel_mut().admitted() {
            return Ok(());
        }
        let registered = self.bootstrap.channel_mut().registered();
        self.shutdown.request_exit_intent(registered)?;
        if !registered {
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
            && !self.bootstrap.qualification_pending()
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
            if self.shutdown.completion_ready(self.lease.is_some()) {
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
        let finite = !self.bootstrap.projection_pending()
            && !self.bootstrap.qualification_pending()
            && !self.main.pending()
            && !self.output.pending();
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

#[cfg(test)]
#[path = "retirement/absence_tests.rs"]
mod absence_tests;
