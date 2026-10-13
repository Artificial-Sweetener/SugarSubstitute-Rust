// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve real queued observation credit across bounded turns and failed host wakes.
use super::kernel::*;
use crate::{
    coordinator_entry::*,
    runtime_test_support::{ClockPort, ManualClock, Rig, TestResult, Wake, spec},
    scope::lock,
};
use std::{
    num::NonZeroUsize,
    sync::{Arc, Mutex, atomic::Ordering},
};
use sugarsubstitute_contracts::execution::*;

fn client(rig: &Rig) -> TestResult<(Scope, Arc<Boundary>, CoordinatorEntryClientSession)> {
    let boundary = Boundary::new(rig, false)?;
    let scope = native_scope(rig)?;
    let mut specification = spec(1, JobClass::NativeObservation, MIN_ENTRY_CLIENT_BYTES)?;
    specification.operation = Operation::NativeLifecycle;
    let session = reserve_client(
        &rig.client.core,
        &scope,
        specification,
        CoordinatorEntryClientInput::new(facts()?, None, request()?, source()?)?,
        Box::new(Lease(boundary.clone())),
    )?;
    Ok((scope, boundary, session))
}

#[test]
fn bounded_server_turn_rearms_remaining_facts_without_new_producer_and_retains_all_frame_loans()
-> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, true)?;
    let (scope, mut session) = server(&rig, &boundary)?;
    activate(&rig, &session)?;
    for index in 0..20 {
        boundary.connect_at(index)?;
    }
    // Registration follows this real native pass publishing all20 original
    // source deadlines; no additional endpoint or producer signal is required.
    rig.clock.wait_until_registered(Moment::from_nanos(29_999_999_981))?;
    assert_eq!(boundary.writes.load(Ordering::Acquire), 20);
    assert_eq!(lock(&session.control.shared.state).observations.len(), 40);
    rig.client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
    let mut loans = Vec::new();
    for _ in 0..32 {
        let fact = session.control.take_observation().ok_or("Actual bounded batch was short")?;
        if let EntryTransportObservation::RequestAuthenticated { request, .. } = fact {
            loans.push(request);
        }
    }
    assert_eq!(loans.len(), 16);
    let observed = rig.wake.count();
    assert!(session.control.rearm_observations());
    assert_eq!(rig.wake.count(), observed + 1);
    assert!(session.control.rearm_observations());
    assert_eq!(rig.wake.count(), observed + 1);
    assert_eq!(boundary.writes.load(Ordering::Acquire), 20);
    assert_eq!(lock(&session.control.shared.state).observations.len(), 8);
    rig.client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
    for _ in 0..8 {
        let fact = session.control.take_observation().ok_or("Original second batch was lost")?;
        if let EntryTransportObservation::RequestAuthenticated { request, .. } = fact {
            loans.push(request);
        }
    }
    assert_eq!(loans.len(), 20);
    assert!(!session.control.rearm_observations());
    assert_eq!(rig.wake.count(), observed + 1);
    assert_eq!(
        lock(&session.control.shared.state)
            .connections
            .iter()
            .map(|slot| slot.observed)
            .sum::<usize>(),
        0
    );
    assert_eq!(scope.snapshot().reserved_bytes, MIN_ENTRY_SERVER_BYTES);
    session.control.close();
    boundary.retiring.wait()?;
    assert!(session.completion.try_take_completion().is_none());
    assert_eq!(boundary.drops.load(Ordering::Acquire), 0);
    assert_eq!(loans[0].view()?.profile(), "profile_1");
    assert!(
        lock(&boundary.memory)
            .as_ref()
            .ok_or("Original server claim missing")?
            .claim(MIN_ENTRY_SERVER_BYTES)
            .is_err()
    );
    drop(loans);
    let completion = rig.completion(&mut session.completion)?;
    assert!(
        matches!(completion.outcome, JobOutcome::Success(report) if report.authenticated_launches == 20)
    );
    assert_eq!(boundary.drops.load(Ordering::Acquire), 1);
    drop(completion);
    drop(session);
    drop(lock(&boundary.memory).take());
    drop(rig.owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn terminal_observation_rearm_preserves_original_failed_wake_and_remaining_facts() -> TestResult {
    let rig = Rig::new()?;
    let (_scope, _boundary, mut session) = client(&rig)?;
    let completion = rig.completion(&mut session.completion)?;
    assert!(matches!(completion.outcome, JobOutcome::Success(_)));
    assert!(lock(&session.control.shared.state).stopped);
    rig.client.drain_completions(NonZeroUsize::MIN)?;
    let queued = lock(&session.control.shared.state).observations.len();
    assert!(queued > 0);
    rig.wake.fail.store(true, Ordering::Release);
    assert!(session.control.rearm_observations());
    rig.wake.fail.store(false, Ordering::Release);
    let original = rig
        .client
        .drain_completions(NonZeroUsize::MIN)
        .err()
        .ok_or("Original rearm wake failure vanished")?;
    assert_eq!(original.operation(), Operation::NativeLifecycle);
    assert!(original.to_string().contains("Owned wake fixture failed"));
    assert_eq!(lock(&session.control.shared.state).observations.len(), queued);
    let mut taken = 0;
    while session.control.take_observation().is_some() {
        taken += 1;
    }
    assert_eq!(taken, queued);
    let observed = rig.wake.count();
    assert!(!session.control.rearm_observations());
    assert_eq!(rig.wake.count(), observed);
    Ok(())
}

#[derive(Default)]
struct ReentrantWake {
    observed: Arc<Wake>,
    action: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}
impl CompletionWake for ReentrantWake {
    fn wake(&self) -> Result<(), ExecutionError> {
        let action = lock(&self.action).take();
        if let Some(action) = action {
            action();
        }
        self.observed.wake()
    }
}

#[test]
fn rearm_delivers_after_session_unlock_even_after_actual_worker_join() -> TestResult {
    let clock = Arc::new(ManualClock::default());
    let wake = Arc::new(ReentrantWake::default());
    let owner = crate::ExecutionOwner::new(Arc::new(ClockPort(clock.clone())), wake.clone())?;
    let execution = owner.client();
    let scope = execution.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let rig = Rig { owner, client: execution, clock, wake: wake.observed.clone(), scope };
    let (_scope, _boundary, session) = client(&rig)?;
    let CoordinatorEntryClientSession { control, mut completion } = session;
    let returned = rig.completion(&mut completion)?;
    assert!(matches!(returned.outcome, JobOutcome::Success(_)));
    drop(rig.owner);
    rig.client.drain_completions(NonZeroUsize::MIN)?;
    let control = Arc::new(control);
    let selected = control.clone();
    let observation = Arc::new(Mutex::new(None));
    let result = observation.clone();
    *lock(&wake.action) = Some(Box::new(move || {
        // The joined worker cannot legitimately contend on this session. A
        // broken under-lock wake is detected without attempting a hanging read.
        let unlocked = selected.shared.state.try_lock().is_ok();
        if unlocked {
            *lock(&result) = selected.take_observation();
        }
    }));
    assert!(control.rearm_observations());
    assert!(matches!(*lock(&observation), Some(EntryTransportObservation::Prepared)));
    Ok(())
}
