// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! A launch waiting on another copy shows something at two seconds on a manual clock, keeps
//! forwarding behind it, and asks its question only when a probe finds that copy hung.
use super::*;
use crate::completion_wake::Wake;
use std::sync::atomic::{AtomicUsize, Ordering};
use sugarsubstitute_contracts::{
    coordinator_entry::{EntryConnectionId, EntryRequestId},
    execution::{ExecutionPort, ScopeId, ScopeSpec},
};
use sugarsubstitute_execution::{ExecutionOwner, clock_fixture::ManualClock};
type Result = std::result::Result<(), Box<dyn Error>>;

const COPY: PreviousCopy = PreviousCopy { pid: 41, birth: 43 };
const OTHER: PreviousCopy = PreviousCopy { pid: 51, birth: 53 };

fn at(millis: u64) -> Moment {
    Moment::from_nanos(millis * 1_000_000)
}
fn received(
    outcome: EntryOutcome,
) -> std::result::Result<EntryTransportObservation, Box<dyn Error>> {
    Ok(EntryTransportObservation::OutcomeReceived {
        connection: EntryConnectionId::new(1)?,
        request: EntryRequestId::new(1)?,
        outcome,
    })
}

/// The real execution owner on a manual clock, with the runner's job sequence.
struct Rig {
    clock: Arc<ManualClock>,
    wake: Arc<Wake>,
    scope: Option<Scope>,
    executor: Option<ExecutionOwner>,
    client: ExecutionClient,
    jobs: crate::job_sequence::JobSequence,
}
impl Rig {
    fn new() -> std::result::Result<Self, Box<dyn Error>> {
        let clock = ManualClock::new();
        let wake = Arc::new(Wake::default());
        let executor = ExecutionOwner::new(clock.clone(), wake.clone())?;
        let client = executor.client();
        let scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        Ok(Self {
            clock,
            wake,
            scope: Some(scope),
            executor: Some(executor),
            client,
            jobs: crate::job_sequence::JobSequence::new(),
        })
    }
    fn now(&self) -> Moment {
        self.client.now().unwrap_or(Moment::from_nanos(0))
    }
    fn advance_to(&self, millis: u64) -> Result {
        self.clock.advance_to(at(millis))?;
        Ok(())
    }
    /// Advance once at the current moment.
    fn step(
        &self,
        wait: &mut ForwardWait,
        port: ProcessPort,
        holder: Option<PreviousCopy>,
    ) -> std::result::Result<WaitStep, Box<dyn Error>> {
        let scope = self.scope.as_ref().ok_or("scope retired")?;
        wait.advance(self.now(), (&self.client, scope, &self.jobs), port, holder)
    }
    /// Advance on each execution wake until `reached`; real work only makes this slower.
    fn until(
        &self,
        wait: &mut ForwardWait,
        port: ProcessPort,
        holder: Option<PreviousCopy>,
        reached: impl Fn(&ForwardWait) -> bool,
    ) -> Result {
        self.wake.until(|| {
            self.client.drain_completions(std::num::NonZeroUsize::MIN).ok()?;
            self.step(wait, port, holder).ok()?;
            reached(wait).then_some(())
        });
        Ok(())
    }
}
impl Drop for Rig {
    fn drop(&mut self) {
        drop(self.scope.take());
        drop(self.executor.take());
    }
}

fn responsive(_: u32, _: u64) -> std::result::Result<ProcessResponsiveness, ProcessError> {
    Ok(ProcessResponsiveness::Responsive)
}
fn hung(_: u32, _: u64) -> std::result::Result<ProcessResponsiveness, ProcessError> {
    Ok(ProcessResponsiveness::Unresponsive)
}
fn never_ends(_: u32, _: u64) -> std::result::Result<bool, ProcessError> {
    Ok(false)
}
fn port(probe: ProbeProcess) -> ProcessPort {
    ProcessPort { probe, end: never_ends }
}

fn phase(surface: &mut WaitSurface) -> ConflictPhase {
    surface.poll().phase
}

/// The copy verified itself on its pipe as serving launches, the way the transport reports it.
fn serving(wait: &mut ForwardWait, copy: PreviousCopy, now: Moment) -> Result {
    wait.discovered(copy);
    wait.serving = true;
    wait.await_answer(now)
}

/// A copy that verified itself on its pipe and then leaves this launch unanswered is asked
/// about at two seconds on the manual execution clock, not before, even while its windows keep
/// answering their message queue; "Close it and start" ends exactly that copy.
#[test]
fn a_silent_server_is_asked_about_at_two_seconds_whatever_its_windows_answer() -> Result {
    let rig = Rig::new()?;
    let mut wait = ForwardWait::new(rig.now())?;
    serving(&mut wait, COPY, rig.now())?;
    rig.advance_to(1_999)?;
    assert_eq!(rig.step(&mut wait, port(responsive), None)?, WaitStep::Continue);
    assert!(!wait.presentation_due(rig.now()));
    assert!(wait.take_surface(rig.now()).is_none());
    assert_eq!(wait.wake_at(), Some(at(2_000)));
    rig.advance_to(2_000)?;
    assert!(wait.presentation_due(rig.now()), "the pump must admit its loop at two seconds");
    let mut surface = wait.take_surface(rig.now()).ok_or("no surface at two seconds")?;
    assert!(wait.take_surface(rig.now()).is_none(), "a wait presents exactly once");
    assert!(!wait.presentation_due(rig.now()), "a taken surface is not due again");
    rig.step(&mut wait, port(responsive), None)?;
    let asked = surface.poll();
    assert_eq!(asked.phase, ConflictPhase::AwaitingDecision);
    assert!(asked.close_enabled());
    surface.choose(ConflictAction::Close);
    let ends = ProcessPort { probe: responsive, end: ends_silent };
    rig.until(&mut wait, ends, None, |wait| matches!(wait.ending, Some(Ending::Ended)))?;
    assert_eq!(ENDED_SILENT.load(Ordering::Acquire), 1);
    assert_eq!(phase(&mut surface), ConflictPhase::Forcing);
    wait.resolve();
    assert!(surface.poll().done());
    assert_eq!(wait.wake_at(), None);
    Ok(())
}

static ENDED_SILENT: AtomicUsize = AtomicUsize::new(0);
fn ends_silent(pid: u32, birth: u64) -> std::result::Result<bool, ProcessError> {
    if (pid, birth) == (COPY.pid, COPY.birth) {
        ENDED_SILENT.fetch_add(1, Ordering::AcqRel);
    }
    Ok(true)
}

/// A copy that answers in time is never asked about: its answer clears the silence, and the
/// checking line stays while its windows answer.
#[test]
fn an_answer_in_time_keeps_the_checking_line() -> Result {
    let rig = Rig::new()?;
    let mut wait = ForwardWait::new(rig.now())?;
    serving(&mut wait, COPY, rig.now())?;
    rig.advance_to(1_500)?;
    wait.observe(
        &received(EntryOutcome::Rejected(
            sugarsubstitute_contracts::coordinator_entry::EntryRejection::Capacity,
        ))?,
        rig.now(),
    )?;
    rig.advance_to(2_000)?;
    let mut surface = wait.take_surface(rig.now()).ok_or("no surface at two seconds")?;
    rig.until(&mut wait, port(responsive), None, |wait| wait.next_probe_at == at(4_000))?;
    assert_eq!(phase(&mut surface), ConflictPhase::Checking);
    surface.choose(ConflictAction::Close);
    assert_eq!(rig.step(&mut wait, port(responsive), None)?, WaitStep::Continue);
    assert!(wait.ending.is_none(), "a copy that answered is never offered for closing");
    wait.resolve();
    assert!(surface.poll().done());
    Ok(())
}

/// The pump admits its loop exactly when the closing line is due, so a closing copy this launch
/// never reached is on screen at two seconds rather than waited for silently.
#[test]
fn a_closing_copy_without_a_handshake_is_due_on_screen_at_two_seconds() -> Result {
    let rig = Rig::new()?;
    let mut wait = ForwardWait::new(rig.now())?;
    wait.reelected();
    rig.advance_to(1_999)?;
    assert!(!wait.presentation_due(rig.now()));
    rig.advance_to(2_000)?;
    assert!(wait.presentation_due(rig.now()));
    wait.resolve();
    assert!(!wait.presentation_due(rig.now()), "a resolved wait never opens the loop");
    Ok(())
}
/// A closing copy that this launch never reached, because its endpoint is gone while it still
/// holds its claim, shows the closing line at two seconds with only Cancel. With no identity
/// to probe it is never asked about, and Cancel ends this launch.
#[test]
fn a_closing_copy_without_a_handshake_shows_the_closing_line_at_two_seconds() -> Result {
    let rig = Rig::new()?;
    let mut wait = ForwardWait::new(rig.now())?;
    wait.reelected();
    rig.advance_to(1_999)?;
    rig.step(&mut wait, port(hung), None)?;
    assert!(wait.take_surface(rig.now()).is_none());
    rig.advance_to(2_000)?;
    let mut surface = wait.take_surface(rig.now()).ok_or("no surface at two seconds")?;
    rig.step(&mut wait, port(hung), None)?;
    let view = surface.poll();
    assert_eq!(view.phase, ConflictPhase::AwaitingExit);
    assert!(!view.close_enabled() && !view.force_enabled() && !view.done());
    surface.choose(ConflictAction::Close);
    assert_eq!(rig.step(&mut wait, port(hung), None)?, WaitStep::Continue);
    assert_eq!(phase(&mut surface), ConflictPhase::AwaitingExit);
    surface.choose(ConflictAction::Cancel);
    assert_eq!(rig.step(&mut wait, port(hung), None)?, WaitStep::Cancel);
    assert!(surface.poll().done());
    Ok(())
}

static ENDED: AtomicUsize = AtomicUsize::new(0);
fn ends(pid: u32, birth: u64) -> std::result::Result<bool, ProcessError> {
    if (pid, birth) == (COPY.pid, COPY.birth) {
        ENDED.fetch_add(1, Ordering::AcqRel);
    }
    Ok(true)
}

/// Without a handshake, once the claims owner names the holder, a probe that finds it hung
/// turns the closing line into the question; "Close it and start" ends exactly that copy, and
/// this launch winning election afterwards dismisses the surface.
#[test]
fn a_hung_claim_holder_gets_the_question_and_closing_it_ends_only_that_copy() -> Result {
    let rig = Rig::new()?;
    let mut wait = ForwardWait::new(rig.now())?;
    wait.reelected();
    rig.advance_to(2_000)?;
    let mut surface = wait.take_surface(rig.now()).ok_or("no surface at two seconds")?;
    let holder = Some(COPY);
    let hung_port = ProcessPort { probe: hung, end: ends };
    rig.until(&mut wait, hung_port, holder, |wait| wait.unresponsive)?;
    let asked = surface.poll();
    assert_eq!(asked.phase, ConflictPhase::AwaitingDecision);
    assert!(asked.close_enabled());
    surface.choose(ConflictAction::Close);
    rig.until(&mut wait, hung_port, holder, |wait| matches!(wait.ending, Some(Ending::Ended)))?;
    assert_eq!(ENDED.load(Ordering::Acquire), 1);
    assert_eq!(phase(&mut surface), ConflictPhase::Forcing);
    wait.resolve();
    assert!(surface.poll().done());
    Ok(())
}

/// A copy that answers Unresponsive itself is asked about at once, before two seconds.
#[test]
fn an_unresponsive_answer_asks_at_once() -> Result {
    let rig = Rig::new()?;
    let mut wait = ForwardWait::new(rig.now())?;
    wait.peer = Some(COPY);
    wait.observe(&received(EntryOutcome::Unresponsive)?, rig.now())?;
    rig.advance_to(100)?;
    let mut surface = wait.take_surface(rig.now()).ok_or("no question for an unresponsive copy")?;
    assert_eq!(phase(&mut surface), ConflictPhase::AwaitingDecision);
    Ok(())
}

/// A copy that answers Closing is waited for without a question: at two seconds the closing
/// line appears, and this launch winning election once that copy has gone dismisses it.
#[test]
fn a_closing_copy_is_waited_for_then_the_launch_continues() -> Result {
    let rig = Rig::new()?;
    let mut wait = ForwardWait::new(rig.now())?;
    wait.peer = Some(COPY);
    wait.observe(&received(EntryOutcome::Closing)?, rig.now())?;
    rig.advance_to(500)?;
    assert!(wait.take_surface(rig.now()).is_none());
    rig.advance_to(2_000)?;
    let mut surface = wait.take_surface(rig.now()).ok_or("no surface at two seconds")?;
    rig.step(&mut wait, port(responsive), None)?;
    assert_eq!(phase(&mut surface), ConflictPhase::AwaitingExit);
    wait.resolve();
    assert!(surface.poll().done());
    Ok(())
}

/// Re-election that reaches a different copy forgets what was known about the old one, so a
/// copy that was asked about never colours the next one.
#[test]
fn a_different_copy_after_reelection_starts_from_nothing() -> Result {
    let rig = Rig::new()?;
    let mut wait = ForwardWait::new(rig.now())?;
    wait.peer = Some(COPY);
    wait.observe(&received(EntryOutcome::Unresponsive)?, rig.now())?;
    assert!(wait.unresponsive);
    wait.peer = Some(COPY);
    wait.reelected();
    wait.discovered(OTHER);
    assert_eq!(wait.peer, Some(OTHER));
    assert!(!wait.unresponsive && !wait.closing);
    Ok(())
}

static FAILED_PROBES: AtomicUsize = AtomicUsize::new(0);
fn failing(_: u32, _: u64) -> std::result::Result<ProcessResponsiveness, ProcessError> {
    FAILED_PROBES.fetch_add(1, Ordering::AcqRel);
    Err(ProcessError::new("probe_process_windows", std::io::Error::other("access denied")))
}

/// A probe that fails is logged and tried again two seconds later; it never ends the launch
/// or calls the copy unresponsive.
#[test]
fn a_failed_probe_is_retried_and_never_ends_the_launch() -> Result {
    let rig = Rig::new()?;
    let mut wait = ForwardWait::new(rig.now())?;
    wait.peer = Some(COPY);
    rig.advance_to(2_000)?;
    let mut surface = wait.take_surface(rig.now()).ok_or("no surface at two seconds")?;
    rig.until(&mut wait, port(failing), None, |wait| wait.next_probe_at == at(4_000))?;
    assert_eq!(FAILED_PROBES.load(Ordering::Acquire), 1);
    assert!(!wait.unresponsive);
    assert_eq!(phase(&mut surface), ConflictPhase::Checking);
    Ok(())
}
