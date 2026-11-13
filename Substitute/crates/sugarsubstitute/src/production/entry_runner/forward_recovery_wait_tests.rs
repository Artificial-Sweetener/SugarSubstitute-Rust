// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The forwarding launch shows the running copy's state at two seconds on a manual clock.
use super::*;
use crate::completion_wake::Wake;
use std::{sync::Arc, time::Duration};
use sugarsubstitute_contracts::{
    coordinator_entry::{EntryConnectionId, EntryRequestId},
    execution::{ExecutionPort, ScopeId, ScopeSpec},
};
use sugarsubstitute_desktop::ProcessError;
use sugarsubstitute_execution::{ExecutionOwner, clock_fixture::ManualClock};
type Result = std::result::Result<(), Box<dyn Error>>;

const COPY: PreviousCopy = PreviousCopy { pid: 41, birth: 43 };

fn at(millis: u64) -> Moment {
    Moment::from_nanos(millis * 1_000_000)
}
fn forwarding() -> std::result::Result<ForwardWait, Box<dyn Error>> {
    let mut wait = ForwardWait::new(at(0))?;
    wait.peer = Some(COPY);
    Ok(wait)
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
fn busy(state: PreviousCopyState) -> Decision {
    Decision::Present(PreviousCopyBusy { copy: COPY, state })
}

/// A copy that never answers is probed at two seconds, never before, and only the probe's
/// answer calls it unresponsive.
#[test]
fn a_silent_copy_is_probed_at_two_seconds_and_only_the_probe_decides() -> Result {
    let wait = forwarding()?;
    assert_eq!(wait.decide(at(1_999), None), Decision::Continue);
    assert_eq!(wait.wake_at(), at(2_000));
    assert_eq!(wait.decide(at(2_000), None), Decision::Probe);
    assert_eq!(
        wait.decide(at(2_000), Some(ProcessResponsiveness::Unresponsive)),
        busy(PreviousCopyState::Unresponsive)
    );
    assert_eq!(wait.decide(at(2_000), Some(ProcessResponsiveness::Responsive)), Decision::Continue);
    assert_eq!(wait.decide(at(60_000), Some(ProcessResponsiveness::Exited)), Decision::Continue);
    Ok(())
}

/// The running copy's own Unresponsive answer is shown at once.
#[test]
fn an_unresponsive_answer_is_presented_immediately() -> Result {
    let mut wait = forwarding()?;
    wait.observe(&received(EntryOutcome::Unresponsive)?);
    assert_eq!(wait.decide(at(100), None), busy(PreviousCopyState::Unresponsive));
    Ok(())
}

/// A closing copy is waited for silently for two seconds, then on screen; a returned source
/// after a serving copy counts as closing, and after an initializing one does not.
#[test]
fn a_closing_copy_is_shown_waiting_after_two_seconds() -> Result {
    let mut wait = forwarding()?;
    wait.observe(&received(EntryOutcome::Closing)?);
    assert_eq!(wait.decide(at(500), None), Decision::Continue);
    assert_eq!(wait.wake_at(), at(2_000));
    assert_eq!(wait.decide(at(2_000), None), busy(PreviousCopyState::Closing));
    let mut reelected = forwarding()?;
    reelected.serving = true;
    reelected.reelected();
    assert_eq!(reelected.decide(at(2_000), None), busy(PreviousCopyState::Closing));
    let mut initializing = forwarding()?;
    initializing.reelected();
    assert_eq!(initializing.decide(at(2_000), None), Decision::Probe);
    Ok(())
}

/// Without a verified copy there is nothing to present or probe, and an accepted launch ends
/// forwarding on its own path.
#[test]
fn nothing_is_presented_without_a_verified_copy() -> Result {
    let mut wait = ForwardWait::new(at(0))?;
    wait.reelected();
    wait.observe(&received(EntryOutcome::Closing)?);
    assert_eq!(
        wait.decide(at(10_000), Some(ProcessResponsiveness::Unresponsive)),
        Decision::Continue
    );
    Ok(())
}

static HUNG_PROBES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
fn hung(_: u32, _: u64) -> std::result::Result<ProcessResponsiveness, ProcessError> {
    HUNG_PROBES.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
    Ok(ProcessResponsiveness::Unresponsive)
}
fn responsive(_: u32, _: u64) -> std::result::Result<ProcessResponsiveness, ProcessError> {
    Ok(ProcessResponsiveness::Responsive)
}

/// A server that verified itself and then never answers surfaces at two seconds on the manual
/// execution clock: nothing is probed before then, the owned probe runs at two seconds, and its
/// hung answer presents the question at once. A responsive answer instead keeps forwarding and
/// probes again two seconds later, because that copy's window comes forward when it answers.
#[test]
fn a_silent_server_surfaces_at_two_seconds_on_the_manual_clock() -> Result {
    let clock = ManualClock::new();
    let wake = Arc::new(Wake::default());
    let executor = ExecutionOwner::new(clock.clone(), wake.clone())?;
    let client = executor.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let jobs = crate::job_sequence::JobSequence::new();
    let mut wait = ForwardWait::new(client.now()?)?;
    wait.peer = Some(COPY);
    clock.advance(Duration::from_millis(1_999))?;
    assert_eq!(wait.advance(client.now()?, (&client, &scope, &jobs), hung)?, None);
    assert_eq!(HUNG_PROBES.load(std::sync::atomic::Ordering::Acquire), 0);
    assert_eq!(wait.wake_at(), at(2_000));
    clock.advance(Duration::from_millis(1))?;
    let shown = wake.until(|| {
        client.drain_completions(std::num::NonZeroUsize::MIN).ok()?;
        wait.advance(client.now().ok()?, (&client, &scope, &jobs), hung).ok().flatten()
    });
    assert_eq!(client.now()?, at(2_000));
    assert_eq!(shown, PreviousCopyBusy { copy: COPY, state: PreviousCopyState::Unresponsive });
    assert_eq!(HUNG_PROBES.load(std::sync::atomic::Ordering::Acquire), 1);
    let mut answering = ForwardWait::new(at(0))?;
    answering.peer = Some(COPY);
    let rescheduled = wake.until(|| {
        client.drain_completions(std::num::NonZeroUsize::MIN).ok()?;
        answering.advance(client.now().ok()?, (&client, &scope, &jobs), responsive).ok()?;
        (answering.wake_at() > at(2_000)).then_some(answering.wake_at())
    });
    assert_eq!(rescheduled, at(4_000));
    drop(scope);
    drop(executor);
    Ok(())
}
