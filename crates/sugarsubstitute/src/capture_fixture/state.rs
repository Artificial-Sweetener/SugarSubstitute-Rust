// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep typed evidence, immutable alarms and coalesced cleanup demand separate.
use super::*;
use sugarsubstitute_contracts::crash::CaptureHandlerIdentity;
/// Preserve matched actual facts even when normal registration was unsuccessful.
pub(super) struct Evidence {
    owner: CoordinatorKey,
    pub(super) handler: Option<CaptureHandlerIdentity>,
    pub(super) registered: bool,
    pub(super) exit: Option<u32>,
    pub(super) closed: bool,
}
impl Evidence {
    /// Bind every future observation to one coordinator lifetime.
    pub(super) const fn new(owner: CoordinatorKey) -> Self {
        Self { owner, handler: None, registered: false, exit: None, closed: false }
    }
    /// Reject stale identities and replay without overwriting retained evidence.
    pub(super) fn observe(&mut self, fact: CaptureObservation) -> Result {
        let owner = match fact {
            CaptureObservation::Registered { owner, .. }
            | CaptureObservation::HandlerExited { owner, .. }
            | CaptureObservation::Closed { owner }
            | CaptureObservation::Degraded { owner, .. } => owner,
        };
        if owner != self.owner || self.closed {
            return Err("Foreign or retired capture fact".into());
        }
        match fact {
            CaptureObservation::Registered { handler, .. } => {
                if self.registered
                    || self.exit.is_some()
                    || handler.process_id == 0
                    || handler.creation_time_100ns == 0
                    || self.handler.is_some_and(|old| old != handler)
                {
                    return Err("Invalid registered capture fact".into());
                }
                self.handler = Some(handler);
                self.registered = true;
            }
            CaptureObservation::HandlerExited { handler, exit_code, .. } => {
                if self.exit.is_some()
                    || handler.process_id == 0
                    || handler.creation_time_100ns == 0
                    || self.handler.is_some_and(|old| old != handler)
                {
                    return Err("Invalid actual capture exit fact".into());
                }
                self.handler = Some(handler);
                self.exit = Some(exit_code);
            }
            CaptureObservation::Closed { .. } => self.closed = true,
            CaptureObservation::Degraded { .. } => {
                return Err("Native capture became degraded".into());
            }
        }
        Ok(())
    }
    /// Require registration, the expected real exit and explicit native closure.
    pub(super) fn complete(&self) -> bool {
        self.registered && self.exit == Some(1) && self.closed
    }
}

/// Bound finite close admissions and rearm only from newly observed progress.
pub(super) struct CleanupDemand {
    ready: bool,
    pending: Option<u64>,
    pub(super) admitted: u16,
    pub(super) last_job: Option<u64>,
    pub(super) successful_terminals: u16,
    pub(super) failed_terminals: u16,
}
impl CleanupDemand {
    /// Begin without cleanup authority until shutdown or actual exit requests it.
    pub(super) const fn new() -> Self {
        Self {
            ready: false,
            pending: None,
            admitted: 0,
            last_job: None,
            successful_terminals: 0,
            failed_terminals: 0,
        }
    }
    /// Coalesce a new policy or lifecycle cleanup request.
    pub(super) fn request(&mut self) {
        self.ready = true;
    }
    /// Consume demand before any admission or native effect.
    pub(super) fn take(&mut self) -> bool {
        std::mem::take(&mut self.ready)
    }
    /// Assign a monotonic close identity under the fixed 256-admission bound.
    pub(super) fn next_job(&self) -> Result<u64> {
        if self.admitted >= 256 {
            return Err("Capture close admission bound exhausted".into());
        }
        Ok(3 + u64::from(self.admitted))
    }
    /// Retain the admitted identity until its exact terminal notice arrives.
    pub(super) fn admitted(&mut self, job: u64) {
        self.admitted += 1;
        self.pending = Some(job);
        self.last_job = Some(job);
    }
    /// Ignore foreign or replayed notices; successful work permits further progress.
    pub(super) fn terminal(&mut self, job: u64, kind: TerminalKind) {
        if self.pending == Some(job) {
            self.pending = None;
            if kind == TerminalKind::Success {
                self.successful_terminals += 1;
            } else {
                self.failed_terminals += 1;
            }
            // This is only new demand. Actual retirement still requires Closed.
            self.ready |= kind == TerminalKind::Success;
        }
    }
    /// Report unobserved admitted work independently of native closure.
    pub(super) fn has_pending(&self) -> bool {
        self.pending.is_some()
    }
    /// Prevent private acknowledgement from deleting required fixture notices.
    pub(super) fn can_observe_capture(
        &self,
        start_admitted: bool,
        start_terminal_seen: bool,
    ) -> bool {
        (!start_admitted || start_terminal_seen) && !self.has_pending()
    }
}

/// Translate once using the later bracket observation, preserving an earlier bound.
pub(super) fn map_deadline(
    deadline: Moment,
    before: Moment,
    native: Instant,
    after: Moment,
) -> Result<Instant> {
    after.checked_duration_since(before)?;
    native
        .checked_add(deadline.checked_duration_since(after)?)
        .ok_or_else(|| "Capture native alarm overflow".into())
}
/// Keep the original shutdown budget and consume each native alarm once.
pub(super) struct StopPlan {
    pub(super) budget: ShutdownBudget,
    grace_alarm: Instant,
    total_alarm: Instant,
    grace_observed: bool,
    pub(super) total_observed: bool,
}
impl StopPlan {
    /// Translate both original bounds with one bracketed clock observation.
    pub(super) fn new(
        started: Moment,
        before: Moment,
        native: Instant,
        after: Moment,
    ) -> Result<Self> {
        let budget = ShutdownBudget::new(started)?;
        Ok(Self {
            budget,
            grace_alarm: map_deadline(budget.graceful_until, before, native, after)?,
            total_alarm: map_deadline(budget.settle_until, before, native, after)?,
            grace_observed: false,
            total_observed: false,
        })
    }
    /// Return each elapsed boundary once without replacing either deadline.
    pub(super) fn due(&mut self, now: Moment) -> (bool, bool) {
        let grace = !self.grace_observed && now >= self.budget.graceful_until;
        let total = !self.total_observed && now >= self.budget.settle_until;
        self.grace_observed |= grace;
        self.total_observed |= total;
        (grace, total)
    }
    /// Remove consumed alarms so unchanged native turns cannot spin on them.
    pub(super) fn next_deadline(&self) -> Option<Instant> {
        if !self.grace_observed {
            Some(self.grace_alarm)
        } else if !self.total_observed {
            Some(self.total_alarm)
        } else {
            None
        }
    }
}
