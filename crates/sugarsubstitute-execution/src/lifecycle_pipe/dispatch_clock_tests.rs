// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise an unlocked clock panic against actual revocation, shutdown and retained storage.
use super::*;
use crate::runtime_test_support::{ClockPort, ManualClock, Wake};
struct IssueClock {
    manual: Arc<ManualClock>,
    boundary: Arc<Boundary>,
    armed: AtomicBool,
    sampled: Gate,
    release: Gate,
}
impl Clock for IssueClock {
    fn now(&self) -> Moment {
        if self.armed.load(Ordering::Acquire)
            && lock(&self.boundary.owner_thread).as_ref() == Some(&std::thread::current().id())
        {
            self.sampled.open();
            if self.release.wait().is_err() {
                std::panic::resume_unwind(Box::new("Clock gate failed"));
            }
            std::panic::resume_unwind(Box::new("Issue clock failed"));
        }
        ClockPort(self.manual.clone()).now()
    }
    fn sleep_until(&self, deadline: Moment) -> WaitFuture {
        ClockPort(self.manual.clone()).sleep_until(deadline)
    }
}
#[test]
fn clock_failure_outside_issue_fence_preserves_shutdown_and_retained_storage() -> TestResult {
    let boundary = Boundary::new();
    boundary.hold_write.store(true, Ordering::Release);
    boundary.allow_retire.store(false, Ordering::Release);
    let manual = Arc::new(ManualClock::default());
    let clock = Arc::new(IssueClock {
        manual: manual.clone(),
        boundary: boundary.clone(),
        armed: AtomicBool::new(false),
        sampled: Gate::default(),
        release: Gate::default(),
    });
    let wake = Arc::new(Wake::default());
    let owner = crate::ExecutionOwner::with_cpu_workers(clock.clone(), wake.clone(), 2)?;
    let client = owner.client();
    let original_scope =
        client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let rig = Rig { owner, client, clock: manual, wake, scope: original_scope };
    let scope = scope(&rig)?;
    let mut session = session(&rig, &scope, &boundary, true)?;
    connect(&rig, &session)?;
    let output = session.control.take_write_buffer()?.ok_or("Missing buffer")?;
    let admission = session
        .control
        .write(LifecyclePipeOperationId::new(1)?, Moment::from_nanos(900), output, 1)
        .map_err(|e| e.error)?;
    boundary.preparing_write.wait()?;
    clock.armed.store(true, Ordering::Release);
    boundary.write_release.open();
    clock.sampled.wait()?;
    // Exercise the very same Core-before-State helper used by public revoke. The
    // provider owns no pipe State; original panic shutdown waits for this Core guard.
    {
        let mut core = lock(&rig.client.core.state);
        clock.release.open();
        assert_eq!(
            admission.revoke_with_core(&mut core),
            NativeWriteRevocation::RevokedBeforeIssue
        );
    }
    boundary.retiring.wait()?;
    assert_eq!(boundary.writes.load(Ordering::Acquire), 0);
    assert!(session.control.take_observation().is_none());
    assert!(session.completion.try_take_completion().is_none());
    let memory = lock(&boundary.memory).take().ok_or("Missing memory")?;
    assert!(memory.claim(1).is_err());
    assert_eq!(boundary.closes.load(Ordering::Acquire), 0);
    boundary.allow_retire.store(true, Ordering::Release);
    boundary.wake(Role::Write)?;
    let JobOutcome::Panicked(cause) = failures::completion(&rig, &mut session)?.outcome else {
        return Err("Expected original clock panic".into());
    };
    assert_eq!(cause.as_str(), "Issue clock failed");
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    Ok(())
}
#[test]
fn deadline_expiring_during_native_preparation_prevents_issue_without_rearming() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    boundary.hold_write.store(true, Ordering::Release);
    let mut session = session(&rig, &scope, &boundary, true)?;
    connect(&rig, &session)?;
    let output = session.control.take_write_buffer()?.ok_or("Missing buffer")?;
    let admission = session
        .control
        .write(LifecyclePipeOperationId::new(1)?, Moment::from_nanos(100), output, 1)
        .map_err(|e| e.error)?;
    boundary.preparing_write.wait()?;
    rig.clock.advance(100);
    boundary.write_release.open();
    assert!(matches!(failures::completion(&rig, &mut session)?.outcome, JobOutcome::TimedOut));
    assert_eq!(boundary.writes.load(Ordering::Acquire), 0);
    assert!(session.control.take_observation().is_none());
    assert_eq!(admission.revoke(), NativeWriteRevocation::Retired);
    Ok(())
}

#[test]
fn earlier_pending_read_deadline_expiring_during_write_preparation_vetoes_native_effect()
-> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    boundary.pending_read.store(true, Ordering::Release);
    boundary.hold_write.store(true, Ordering::Release);
    let mut session = session(&rig, &scope, &boundary, true)?;
    connect(&rig, &session)?;
    session.control.read(LifecyclePipeOperationId::new(1)?, Moment::from_nanos(100))?;
    fact(&rig, &session.control, |f| matches!(f, LifecyclePipeObservation::ReadPending { .. }))?;
    let output = session.control.take_write_buffer()?.ok_or("Missing buffer")?;
    let admission = session
        .control
        .write(LifecyclePipeOperationId::new(2)?, Moment::from_nanos(900), output, 1)
        .map_err(|e| e.error)?;
    boundary.preparing_write.wait()?;
    rig.clock.advance(100);
    boundary.write_release.open();
    assert!(matches!(failures::completion(&rig, &mut session)?.outcome, JobOutcome::TimedOut));
    assert_eq!(boundary.writes.load(Ordering::Acquire), 0);
    assert!(session.control.take_observation().is_none());
    assert_eq!(admission.revoke(), NativeWriteRevocation::Retired);
    Ok(())
}

#[test]
fn narrowed_child_handshake_expires_before_later_observed_authority_without_native_connect()
-> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new();
    *lock(&boundary.original_counter) = Some(LifecyclePipeDeadline::new(1000, 1_000_000_000)?);
    boundary.hold_connect.store(true, Ordering::Release);
    let mut specification = spec(1, JobClass::NativeObservation, MIN_LIFECYCLE_PIPE_BYTES)?;
    specification.operation = Operation::NativeLifecycle;
    let mut session = reserve(
        &rig.client.core,
        &scope,
        specification,
        Some(run()?),
        Moment::from_nanos(900),
        Box::new(Lease(boundary.clone())),
    )?;
    session.control.bind_input(PeerInput::Simulated);
    boundary.preparing_connect.wait()?;
    assert_eq!(session.control.original_deadline(), Moment::from_nanos(900));
    assert_eq!(
        lock(&session.control.inner.shared.state).deadline,
        Some(LifecyclePipeDeadline::new(1000, 1_000_000_000)?)
    );
    rig.clock.advance(900);
    boundary.connect_release.open();
    assert!(matches!(failures::completion(&rig, &mut session)?.outcome, JobOutcome::TimedOut));
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    while let Some(observed) = session.control.take_observation() {
        assert!(!matches!(observed, LifecyclePipeObservation::Connected));
    }
    Ok(())
}
