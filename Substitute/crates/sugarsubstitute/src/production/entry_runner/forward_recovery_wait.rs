// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Put something on screen whenever a launch forwarded to another copy waits two seconds.
//!
//! A responsive copy answers within a moment and its window comes forward. Otherwise, two
//! seconds after this launch first found another copy, on the execution clock, it shows a
//! waiting surface beside its own pump and keeps forwarding and re-electing behind it: an
//! accepted launch or this launch winning election dismisses the surface. While it waits, a
//! copy that is closing shows "Finishing closing the previous window…", any other wait shows
//! the checking line, and only Cancel is offered. The surface becomes the question "Close it
//! and start" / "Cancel" when the copy has stopped responding: it verified itself on its pipe
//! and then left this launch unanswered for two seconds, it answered Unresponsive, or an owned
//! probe of its windows found them hung. Only that choice ends the verified copy.
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
    /// When the verified copy must have answered this launch; cleared by any answer.
    answer_due: Option<Moment>,
    /// The verified copy let `answer_due` pass without answering.
    silent: bool,
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
            answer_due: None,
            silent: false,
            probe: ResponsivenessProbe::default(),
            next_probe_at: present_at,
            shared: None,
            ending: None,
            resolved: false,
            native_wake: None,
        })
    }
    /// Record what the transport verified or received at `now`.
    /// # Errors
    /// Returns the clock's overflow cause.
    pub(crate) fn observe(
        &mut self,
        observation: &EntryTransportObservation,
        now: Moment,
    ) -> Result<(), Box<dyn Error>> {
        match observation {
            EntryTransportObservation::ConnectionVerified { peer, .. } => {
                self.discovered(copy_of(peer));
                self.await_answer(now)?;
            }
            EntryTransportObservation::DiscoveryVerified { peer, phase, .. } => {
                self.discovered(copy_of(peer));
                if matches!(phase, EntryDiscoveryPhase::Serving(_)) {
                    self.serving = true;
                    self.await_answer(now)?;
                } else {
                    // A copy that is still starting answered; its windows are probed instead.
                    self.answered();
                }
            }
            EntryTransportObservation::OutcomeReceived { outcome, .. } => {
                self.answered();
                match outcome {
                    EntryOutcome::Closing => self.closing = true,
                    EntryOutcome::Unresponsive => self.unresponsive = true,
                    EntryOutcome::Accepted(_) | EntryOutcome::Rejected(_) => {}
                }
            }
            _ => {}
        }
        Ok(())
    }
    /// The transport verified `copy` on its pipe.
    fn discovered(&mut self, copy: PreviousCopy) {
        if self.peer.is_some_and(|known| known != copy) {
            // Re-election reached another copy: what was known about the old one, including a
            // finished close, says nothing about this one.
            self.unresponsive = false;
            self.closing = false;
            self.serving = false;
            self.answered();
            if matches!(self.ending, Some(Ending::Ended | Ending::Failed)) {
                self.ending = None;
            }
        }
        self.peer = Some(copy);
    }
    /// The verified copy now owes this launch an answer; reconnecting keeps the first moment.
    fn await_answer(&mut self, now: Moment) -> Result<(), Box<dyn Error>> {
        if self.answer_due.is_none() {
            self.answer_due = Some(now.checked_add(PRESENTATION_DELAY)?);
        }
        Ok(())
    }
    fn answered(&mut self) {
        self.answer_due = None;
        self.silent = false;
    }
    /// Note whether the verified copy has let its answer fall due.
    fn observe_silence(&mut self, now: Moment) {
        if !self.silent && self.answer_due.is_some_and(|due| now >= due) {
            tracing::warn!(
                operation = "entry_forward_silent",
                "The other copy verified itself and then did not answer this launch"
            );
            self.silent = true;
        }
    }
    /// The launch source came back for re-election. A copy that was serving, or one this
    /// launch never reached because its endpoint is gone while it still holds its claim, is
    /// closing.
    pub(crate) fn reelected(&mut self) {
        self.closing |= self.serving || self.peer.is_none();
        self.answered();
    }
    /// The wait is over: the copy took this launch, or this launch won election. The surface
    /// closes on its next poll.
    pub(crate) fn resolve(&mut self) {
        self.resolved = true;
        self.native_wake = None;
        self.set_phase(ConflictPhase::Done);
    }
    /// Whether the surface is due and not yet taken: the wait has lasted two seconds, or the
    /// copy is known not to respond.
    pub(crate) fn presentation_due(&mut self, now: Moment) -> bool {
        self.observe_silence(now);
        !self.resolved
            && self.shared.is_none()
            && (now >= self.present_at || self.unresponsive || self.silent)
    }
    /// Take the surface's controller once it is due. Returns it exactly once.
    pub(crate) fn take_surface(&mut self, now: Moment) -> Option<WaitSurface> {
        if !self.presentation_due(now) {
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
        if self.unresponsive || self.silent {
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
    /// The copy to probe or end: the one the pipe verified, or the process the coordinator
    /// record names as its holder.
    fn target(&self, holder: Option<PreviousCopy>) -> Option<PreviousCopy> {
        self.peer.or(holder)
    }
    /// Advance at `now`: act on the user's choice, collect an ending, note a verified copy that
    /// let its answer fall due, probe a copy that has not answered, and update what the surface
    /// shows. `holder` is the process the coordinator record names, for a copy this launch
    /// never reached. Probe failures are logged and the copy is probed again later; they never
    /// end the launch.
    /// # Errors
    /// Preserves job submission and clock failures.
    pub(crate) fn advance(
        &mut self,
        now: Moment,
        jobs: Jobs<'_>,
        port: ProcessPort,
        holder: Option<PreviousCopy>,
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
        self.observe_silence(now);
        if self.ending.is_none()
            && !self.unresponsive
            && !self.silent
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
        let asking = self.unresponsive || self.silent;
        let probe = (self.ending.is_none() && !asking).then_some(self.next_probe_at);
        let answer = self.answer_due.filter(|_| !self.silent);
        present.into_iter().chain(probe).chain(answer).min()
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

fn copy_of(peer: &sugarsubstitute_execution::VerifiedEntryPeer) -> PreviousCopy {
    PreviousCopy { pid: peer.process_id(), birth: peer.creation_time_100ns() }
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
