// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decide when a launch forwarded to a running copy needs something on screen.
//!
//! A responsive copy answers within a moment and its window comes forward. A copy that says
//! it is not responding is asked about at once. Otherwise the launch keeps forwarding for two
//! seconds; after that a copy that is closing is waited for on screen, and a copy that has not
//! answered is probed. Only the probe, never the elapsed time, calls a copy unresponsive.
use crate::production::entry_activation::{ProbeProcess, ResponsivenessProbe};
use std::{error::Error, fmt, time::Duration};
use sugarsubstitute_contracts::{
    coordinator_entry::{EntryDiscoveryPhase, EntryOutcome},
    execution::{Moment, Scope},
};
use sugarsubstitute_desktop::ProcessResponsiveness;
use sugarsubstitute_execution::{EntryTransportObservation, ExecutionClient};

/// How long a launch forwards before the user sees why it has not started.
pub(super) const PRESENTATION_DELAY: Duration = Duration::from_secs(2);

/// The kernel-verified identity of the running copy this launch forwarded to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PreviousCopy {
    /// Process id observed on the authenticated pipe.
    pub(crate) pid: u32,
    /// Process creation time in 100 ns units, so a reused id never matches.
    pub(crate) birth: u64,
}

/// Why the running copy did not take this launch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PreviousCopyState {
    /// Its window failed a responsiveness probe or its watchdog suspects it.
    Unresponsive,
    /// It is closing; this launch starts once it has exited.
    Closing,
}

/// The forwarded launch ended so the user can decide about, or wait for, the running copy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PreviousCopyBusy {
    /// The verified running copy.
    pub(crate) copy: PreviousCopy,
    /// What the running copy is doing.
    pub(crate) state: PreviousCopyState,
}
impl PreviousCopyBusy {
    /// Find this cause in a failure chain, at most sixteen causes deep.
    pub(crate) fn find(cause: &(dyn Error + 'static)) -> Option<Self> {
        let mut current = Some(cause);
        for _ in 0..16 {
            let cause = current?;
            if let Some(busy) = cause.downcast_ref::<Self>() {
                return Some(*busy);
            }
            current = cause.source();
        }
        None
    }
}
impl fmt::Display for PreviousCopyBusy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.state {
            PreviousCopyState::Unresponsive => {
                f.write_str("The running copy is not responding to this launch")
            }
            PreviousCopyState::Closing => f.write_str("The running copy is closing"),
        }
    }
}
impl Error for PreviousCopyBusy {}

/// What the forwarding launch does at one moment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Decision {
    /// Keep forwarding without anything on screen.
    Continue,
    /// Probe the running copy, which has not answered within the presentation delay.
    Probe,
    /// End forwarding and show the user the running copy's state.
    Present(PreviousCopyBusy),
}

/// Forwarding progress since this launch first found a running copy.
pub(crate) struct ForwardWait {
    present_at: Moment,
    peer: Option<PreviousCopy>,
    /// The verified copy was serving launches, so losing it means it is closing.
    serving: bool,
    closing: bool,
    unresponsive: bool,
    probe: ResponsivenessProbe,
    next_probe_at: Moment,
    native_wake: Option<std::time::Instant>,
}
impl ForwardWait {
    /// Begin waiting at `started`, the moment the first forward was reserved.
    /// # Errors
    /// Returns the clock's overflow cause.
    pub(crate) fn new(started: Moment) -> Result<Self, Box<dyn Error>> {
        let present_at = started.checked_add(PRESENTATION_DELAY)?;
        Ok(Self {
            present_at,
            peer: None,
            serving: false,
            closing: false,
            unresponsive: false,
            probe: ResponsivenessProbe::default(),
            next_probe_at: present_at,
            native_wake: None,
        })
    }
    /// Record what the transport verified or received.
    pub(crate) fn observe(&mut self, observation: &EntryTransportObservation) {
        match observation {
            // An initializing copy is the same verified process; probing it at two seconds
            // finds one that hangs before it serves.
            EntryTransportObservation::DiscoveryVerified { peer, phase, .. } => {
                self.serving |= matches!(phase, EntryDiscoveryPhase::Serving(_));
                self.peer = Some(PreviousCopy {
                    pid: peer.process_id(),
                    birth: peer.creation_time_100ns(),
                });
            }
            EntryTransportObservation::OutcomeReceived { outcome, .. } => match outcome {
                EntryOutcome::Closing => self.closing = true,
                EntryOutcome::Unresponsive => self.unresponsive = true,
                EntryOutcome::Accepted(_) | EntryOutcome::Rejected(_) => {}
            },
            _ => {}
        }
    }
    /// The launch source came back for re-election: a serving copy closed or dropped it.
    pub(crate) fn reelected(&mut self) {
        self.closing |= self.serving;
    }
    /// Decide at `now`, given the latest completed probe of a silent copy.
    pub(crate) fn decide(&self, now: Moment, probed: Option<ProcessResponsiveness>) -> Decision {
        let Some(copy) = self.peer else { return Decision::Continue };
        let present = |state| Decision::Present(PreviousCopyBusy { copy, state });
        if self.unresponsive || probed == Some(ProcessResponsiveness::Unresponsive) {
            return present(PreviousCopyState::Unresponsive);
        }
        if now < self.present_at {
            return Decision::Continue;
        }
        if self.closing {
            return present(PreviousCopyState::Closing);
        }
        if probed.is_none() && now >= self.next_probe_at {
            Decision::Probe
        } else {
            Decision::Continue
        }
    }
    /// Advance at `now`: probe a silent copy through an owned blocking job and return the
    /// presentation once one is due. A responsive copy is probed again after another delay.
    /// # Errors
    /// Preserves the probe job's submission or failure cause.
    pub(crate) fn advance(
        &mut self,
        now: Moment,
        (client, scope, jobs): (&ExecutionClient, &Scope, &crate::job_sequence::JobSequence),
        probe: ProbeProcess,
    ) -> Result<Option<PreviousCopyBusy>, Box<dyn Error>> {
        let mut probed = None;
        if let Some(copy) = self.peer
            && now >= self.next_probe_at
            && !self.closing
            && !self.unresponsive
        {
            probed = self.probe.observe((copy.pid, copy.birth), client, scope, jobs, probe)?;
            if probed == Some(ProcessResponsiveness::Responsive) {
                self.probe.reset();
                self.next_probe_at = now.checked_add(PRESENTATION_DELAY)?;
            }
        }
        Ok(match self.decide(now, probed) {
            Decision::Present(busy) => Some(busy),
            Decision::Continue | Decision::Probe => None,
        })
    }
    /// The next moment this owner must be consulted, on the execution clock.
    /// A probe answer completes its own job and wakes the runner, so it needs no moment here.
    pub(crate) fn wake_at(&self) -> Moment {
        if self.closing { self.present_at } else { self.next_probe_at.max(self.present_at) }
    }
    /// Anchor the next consultation on the native pump's clock, sampled with `now`.
    pub(crate) fn schedule_native(&mut self, now: Moment, native_now: std::time::Instant) {
        let remaining = self.wake_at().checked_duration_since(now).unwrap_or_default();
        self.native_wake = native_now.checked_add(remaining);
    }
    /// The native pump's next wake for this owner.
    pub(crate) fn native_wake(&self) -> Option<std::time::Instant> {
        self.native_wake
    }
}

#[cfg(test)]
#[path = "forward_recovery_wait_tests.rs"]
mod tests;
