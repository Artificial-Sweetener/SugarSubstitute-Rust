// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Put something on screen whenever a launch forwarded to another copy waits two seconds.
//!
//! A responsive copy answers within a moment and its window comes forward. Otherwise, two
//! seconds after this launch first found another copy, on the execution clock, it shows a
//! waiting surface beside its own pump and keeps forwarding and re-electing behind it: an
//! accepted launch or this launch winning election dismisses the surface. While it waits, a
//! copy that is closing shows "Finishing closing the previous window…", any other wait shows
//! the checking line, and only Cancel is offered. Only an owned responsiveness probe, or the
//! copy's own Unresponsive answer, turns the surface into the question "Close it and start" /
//! "Cancel", and only that choice ends the verified copy.
use crate::production::entry_activation::{ProbeProcess, ResponsivenessProbe};
use std::{
    error::Error,
    sync::{Arc, Mutex, PoisonError},
    time::Duration,
};
use sugarsubstitute_contracts::{
    coordinator_entry::{EntryDiscoveryPhase, EntryOutcome},
    execution::{
        CompletionWake, ExecutionPort, JobClass, JobOutcome, JobSpec, Moment, Operation, OwnedJob,
        Scope,
    },
    instance_conflict::{ConflictAction, ConflictController, ConflictPhase, ConflictView},
};
use sugarsubstitute_desktop::{ProcessError, ProcessResponsiveness};
use sugarsubstitute_execution::{EntryTransportObservation, ExecutionClient};

/// How long a launch forwards before the user sees what it is waiting for.
pub(super) const PRESENTATION_DELAY: Duration = Duration::from_secs(2);
/// Ending a verified copy is one native call; this bounds the job, not the decision.
const END_JOB_BOUND: Duration = Duration::from_secs(30);

/// The kernel-verified identity of the running copy this launch forwarded to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PreviousCopy {
    /// Process id observed on the authenticated pipe.
    pub(crate) pid: u32,
    /// Process creation time in 100 ns units, so a reused id never matches.
    pub(crate) birth: u64,
}

/// Names the process holding the coordinator claim when no handshake verified one.
///
/// The claims owner does not yet expose that identity, so production answers `None` and such
/// a wait shows only the waiting line. Tests supply a holder to prove the question follows.
pub(crate) trait ClaimHolder {
    /// The holder's identity, when the claims owner can name it.
    fn holder(&self) -> Option<PreviousCopy>;
}
/// The production claims owner cannot name the holder yet.
pub(crate) struct UnnamedClaimHolder;
impl ClaimHolder for UnnamedClaimHolder {
    fn holder(&self) -> Option<PreviousCopy> {
        None
    }
}

/// The native operations on a verified copy; tests substitute their own.
#[derive(Clone, Copy)]
pub(crate) struct ProcessPort {
    /// Ask every window of the process to answer, at most one second each.
    pub(crate) probe: ProbeProcess,
    /// End the process outright; false when it had already exited.
    pub(crate) end: fn(u32, u64) -> Result<bool, ProcessError>,
}
/// The desktop adapter's process operations.
pub(crate) const NATIVE_PROCESS: ProcessPort = ProcessPort {
    probe: sugarsubstitute_desktop::probe_process_responsiveness,
    end: sugarsubstitute_desktop::end_process,
};

/// What the runner does after one advance.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum WaitStep {
    /// Keep forwarding.
    Continue,
    /// The user cancelled this launch; it ends without starting.
    Cancel,
}

/// State shared with the surface; the surface reads it and records the user's choice.
#[derive(Debug)]
struct Shared {
    phase: ConflictPhase,
    choice: Option<ConflictAction>,
}

/// The surface's view of this wait. It only reports the runner's phase and records choices;
/// every effect runs in the runner's owned jobs.
pub(crate) struct WaitSurface(Arc<Mutex<Shared>>);
impl ConflictController for WaitSurface {
    fn start(&mut self, _: Arc<dyn CompletionWake>) {}
    fn poll(&mut self) -> ConflictView {
        ConflictView { phase: lock(&self.0).phase }
    }
    fn choose(&mut self, action: ConflictAction) {
        let mut shared = lock(&self.0);
        let admitted = match action {
            ConflictAction::Cancel => shared.phase != ConflictPhase::Done,
            ConflictAction::Close => shared.phase == ConflictPhase::AwaitingDecision,
            ConflictAction::Force => false,
        };
        if admitted && shared.choice.is_none() {
            shared.choice = Some(action);
        }
    }
}
fn lock(shared: &Mutex<Shared>) -> std::sync::MutexGuard<'_, Shared> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Forwarding progress since this launch first found another copy.
pub(crate) struct ForwardWait {
    present_at: Moment,
    peer: Option<PreviousCopy>,
    /// The verified copy was serving launches, so losing it means it is closing.
    serving: bool,
    closing: bool,
    unresponsive: bool,
    probe: ResponsivenessProbe,
    next_probe_at: Moment,
    /// The surface, once presented; `None` until then.
    shared: Option<Arc<Mutex<Shared>>>,
    /// The copy the user chose to close, while that is in progress or failed.
    ending: Option<Ending>,
    resolved: bool,
    native_wake: Option<std::time::Instant>,
}
enum Ending {
    Running(OwnedJob<bool, ProcessError>),
    Ended,
    Failed,
}

/// The execution resources an advance submits owned jobs through.
pub(crate) type Jobs<'a> = (&'a ExecutionClient, &'a Scope, &'a crate::job_sequence::JobSequence);

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
            shared: None,
            ending: None,
            resolved: false,
            native_wake: None,
        })
    }
    /// Record what the transport verified or received.
    pub(crate) fn observe(&mut self, observation: &EntryTransportObservation) {
        match observation {
            EntryTransportObservation::DiscoveryVerified { peer, phase, .. } => self.discovered(
                PreviousCopy { pid: peer.process_id(), birth: peer.creation_time_100ns() },
                matches!(phase, EntryDiscoveryPhase::Serving(_)),
            ),
            EntryTransportObservation::OutcomeReceived { outcome, .. } => match outcome {
                EntryOutcome::Closing => self.closing = true,
                EntryOutcome::Unresponsive => self.unresponsive = true,
                EntryOutcome::Accepted(_) | EntryOutcome::Rejected(_) => {}
            },
            _ => {}
        }
    }
    /// The transport verified `copy` on its pipe, serving launches or still initializing.
    fn discovered(&mut self, copy: PreviousCopy, serving: bool) {
        if self.peer.is_some_and(|known| known != copy) {
            // Re-election reached another copy: what was known about the old one, including a
            // finished close, says nothing about this one.
            self.unresponsive = false;
            self.closing = false;
            self.serving = false;
            if matches!(self.ending, Some(Ending::Ended | Ending::Failed)) {
                self.ending = None;
            }
        }
        self.serving |= serving;
        self.peer = Some(copy);
    }
    /// The launch source came back for re-election. A copy that was serving, or one this
    /// launch never reached because its endpoint is gone while it still holds its claim, is
    /// closing.
    pub(crate) fn reelected(&mut self) {
        self.closing |= self.serving || self.peer.is_none();
    }
    /// The wait is over: the copy took this launch, or this launch won election. The surface
    /// closes on its next poll.
    pub(crate) fn resolve(&mut self) {
        self.resolved = true;
        self.native_wake = None;
        self.set_phase(ConflictPhase::Done);
    }
    /// Take the surface's controller once the wait has lasted two seconds, or at once when the
    /// copy is known not to respond. Returns it exactly once.
    pub(crate) fn take_surface(&mut self, now: Moment) -> Option<WaitSurface> {
        if self.resolved || self.shared.is_some() || (now < self.present_at && !self.unresponsive) {
            return None;
        }
        let shared = Arc::new(Mutex::new(Shared { phase: self.phase(), choice: None }));
        self.shared = Some(shared.clone());
        Some(WaitSurface(shared))
    }
    /// The phase the surface shows now.
    fn phase(&self) -> ConflictPhase {
        if self.resolved {
            return ConflictPhase::Done;
        }
        match self.ending {
            Some(Ending::Failed) => return ConflictPhase::Failed,
            Some(Ending::Running(_) | Ending::Ended) => return ConflictPhase::Forcing,
            None => {}
        }
        if self.unresponsive {
            ConflictPhase::AwaitingDecision
        } else if self.closing {
            ConflictPhase::AwaitingExit
        } else {
            ConflictPhase::Checking
        }
    }
    fn set_phase(&self, phase: ConflictPhase) {
        if let Some(shared) = &self.shared {
            lock(shared).phase = phase;
        }
    }
    /// The copy to probe or end: the one the pipe verified, or the claim holder.
    fn target(&self, holder: &dyn ClaimHolder) -> Option<PreviousCopy> {
        self.peer.or_else(|| holder.holder())
    }
    /// Advance at `now`: act on the user's choice, collect an ending, probe a copy that has not
    /// answered, and update what the surface shows. Probe failures are logged and the copy is
    /// probed again later; they never end the launch.
    /// # Errors
    /// Preserves job submission and clock failures.
    pub(crate) fn advance(
        &mut self,
        now: Moment,
        jobs: Jobs<'_>,
        port: ProcessPort,
        holder: &dyn ClaimHolder,
    ) -> Result<WaitStep, Box<dyn Error>> {
        if self.resolved {
            return Ok(WaitStep::Continue);
        }
        let choice = self.shared.as_ref().and_then(|shared| lock(shared).choice.take());
        match choice {
            Some(ConflictAction::Cancel) => {
                self.resolve();
                return Ok(WaitStep::Cancel);
            }
            Some(ConflictAction::Close) if self.ending.is_none() => {
                if let Some(copy) = self.target(holder) {
                    self.ending = Some(Ending::Running(submit_end(copy, jobs, port)?));
                }
            }
            _ => {}
        }
        self.collect_ending();
        if self.ending.is_none()
            && !self.unresponsive
            && now >= self.next_probe_at
            && let Some(copy) = self.target(holder)
        {
            let (client, scope, sequence) = jobs;
            match self.probe.observe((copy.pid, copy.birth), client, scope, sequence, port.probe) {
                Ok(Some(ProcessResponsiveness::Unresponsive)) => self.unresponsive = true,
                Ok(Some(ProcessResponsiveness::Responsive | ProcessResponsiveness::Exited)) => {
                    self.probe.reset();
                    self.next_probe_at = now.checked_add(PRESENTATION_DELAY)?;
                }
                Ok(None) => {}
                Err(cause) => {
                    tracing::warn!(operation = "entry_forward_probe", cause = %cause,
                        "The other copy could not be probed; it is probed again shortly");
                    self.probe.reset();
                    self.next_probe_at = now.checked_add(PRESENTATION_DELAY)?;
                }
            }
        }
        self.set_phase(self.phase());
        Ok(WaitStep::Continue)
    }
    fn collect_ending(&mut self) {
        let Some(Ending::Running(job)) = &mut self.ending else { return };
        let Some(completion) = job.try_take_completion() else { return };
        self.ending = Some(match completion.outcome {
            JobOutcome::Success(_) => Ending::Ended,
            JobOutcome::Failure(cause) => {
                tracing::error!(operation = "entry_end_previous_copy", cause = %cause,
                    "The previous copy could not be closed");
                Ending::Failed
            }
            _ => {
                tracing::error!(
                    operation = "entry_end_previous_copy",
                    "Closing the previous copy did not complete"
                );
                Ending::Failed
            }
        });
    }
    /// The next moment this owner must be consulted, on the execution clock. Probe and ending
    /// jobs wake the runner when they complete, so they need no moment here.
    pub(crate) fn wake_at(&self) -> Option<Moment> {
        if self.resolved {
            return None;
        }
        let present = self.shared.is_none().then_some(self.present_at);
        let probe = (self.ending.is_none() && !self.unresponsive).then_some(self.next_probe_at);
        present.into_iter().chain(probe).min()
    }
    /// Anchor the next consultation on the native pump's clock, sampled with `now`.
    pub(crate) fn schedule_native(&mut self, now: Moment, native_now: std::time::Instant) {
        self.native_wake = self.wake_at().and_then(|at| {
            native_now.checked_add(at.checked_duration_since(now).unwrap_or_default())
        });
    }
    /// The native pump's next wake for this owner.
    pub(crate) fn native_wake(&self) -> Option<std::time::Instant> {
        self.native_wake
    }
}

/// End the verified copy in an owned blocking job; the end re-verifies its identity first.
fn submit_end(
    copy: PreviousCopy,
    (client, scope, sequence): Jobs<'_>,
    port: ProcessPort,
) -> Result<OwnedJob<bool, ProcessError>, Box<dyn Error>> {
    tracing::warn!(
        operation = "entry_end_previous_copy",
        process_id = copy.pid,
        "The user chose to close the previous copy that is not responding"
    );
    Ok(client.submit_blocking(
        scope,
        JobSpec {
            id: sequence.allocate()?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(client.now()?.checked_add(END_JOB_BOUND)?),
            reservation_bytes: 4096,
            replacement: None,
        },
        move |_| (port.end)(copy.pid, copy.birth),
    )?)
}

#[cfg(test)]
#[path = "forward_recovery_wait_tests.rs"]
mod tests;
