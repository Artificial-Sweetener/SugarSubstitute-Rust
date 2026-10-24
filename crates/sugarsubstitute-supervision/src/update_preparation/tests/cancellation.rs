// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Cancellation and admission faults retain custody until actual cleanup observation.
use super::*;
use std::sync::{Condvar, Mutex};
use sugarsubstitute_contracts::execution::{ExecutionPort, ScopeId, ScopeSpec};

#[test]
fn retained_preparation_cannot_move_to_another_execution_owners_memory_ledger() -> TestResult {
    let mut original = Pipeline::new()?;
    let (fixture, _) = initialized(&mut original)?;
    let mut worker =
        Worker::new(configuration(&fixture, sugarsubstitute_update::http::Policy::loopback(1)?))?;
    step(&mut original, &mut worker)?;
    let mut foreign = Pipeline::new()?;
    assert!(submit(&mut foreign, &mut worker).is_err());
    assert_eq!(worker.progress(), Progress::Checking);
    assert!(!worker.pending());
    worker.cancel();
    assert!(submit(&mut foreign, &mut worker).is_err());
    retire(&mut original, &mut worker)?;
    assert_unlocked(&mut original, &fixture)?;
    Ok(())
}

#[test]
fn cancellation_at_each_preparation_boundary_releases_setup_and_memory_custody() -> TestResult {
    for completed in 2..=5 {
        let mut pipeline = Pipeline::new()?;
        let (fixture, signed) = initialized(&mut pipeline)?;
        let (mut worker, server) = campaign(&mut pipeline, &fixture, signed, completed >= 4)?;
        for _ in 0..completed {
            step(&mut pipeline, &mut worker)?;
        }
        server.finish()?;
        worker.cancel();
        worker.cancel();
        assert_eq!(worker.progress(), Progress::Retiring);
        assert!(worker.take_candidate().is_err());
        retire(&mut pipeline, &mut worker)?;
        assert!(submit(&mut pipeline, &mut worker).is_err());
        assert_unlocked(&mut pipeline, &fixture)?;
    }
    Ok(())
}

#[test]
fn rejected_admission_preserves_phase_and_cleanup_uses_an_independent_live_scope() -> TestResult {
    let mut pipeline = Pipeline::new()?;
    let (fixture, _) = initialized(&mut pipeline)?;
    let mut worker =
        Worker::new(configuration(&fixture, sugarsubstitute_update::http::Policy::loopback(1)?))?;
    assert!(
        pipeline
            .admit(|client, scope, id| Ok(worker.submit(
                client,
                scope,
                id,
                Moment::from_nanos(0),
                || NOW
            )?))
            .is_err()
    );
    assert_eq!(worker.progress(), Progress::Opening);
    assert!(!worker.pending());
    step(&mut pipeline, &mut worker)?;
    let cleanup = pipeline.admit(|client, scope, _| {
        scope.cancel();
        Ok(client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?)
    })?;
    worker.cancel();
    assert!(submit(&mut pipeline, &mut worker).is_err());
    assert_eq!(worker.progress(), Progress::Retiring);
    pipeline.admit(|client, _, id| {
        Ok(worker.submit(client, &cleanup, id, Moment::from_nanos(2_000_000_000), || NOW)?)
    })?;
    let observed = pipeline.observe(|| worker.poll())?;
    assert_eq!(observed.progress, Progress::Closed);
    assert!(matches!(observed.outcome, JobOutcome::Success(())));
    // A separate owner now sees that native setup/run exclusions actually became available.
    let mut check = Pipeline::new()?;
    assert_unlocked(&mut check, &fixture)?;
    Ok(())
}

/// Gate the injected authentication clock, never the implementation's cancellation behavior.
#[derive(Default)]
struct ClockGate(Mutex<(bool, bool)>, Condvar);
impl ClockGate {
    fn now(&self) -> i64 {
        let mut state = crate::registry::lock(&self.0);
        state.0 = true;
        self.1.notify_all();
        match self.1.wait_timeout_while(state, Duration::from_secs(10), |state| !state.1) {
            Ok((state, timeout)) if state.1 && !timeout.timed_out() => NOW,
            _ => i64::MAX,
        }
    }
    fn entered(&self) -> TestResult {
        let (state, timeout) = self
            .1
            .wait_timeout_while(crate::registry::lock(&self.0), Duration::from_secs(10), |state| {
                !state.0
            })
            .map_err(|_| "Clock gate poisoned")?;
        if !state.0 || timeout.timed_out() {
            return Err("Clock gate was not entered".into());
        }
        Ok(())
    }
    fn release(&self) {
        crate::registry::lock(&self.0).1 = true;
        self.1.notify_all();
    }
}

#[test]
fn cancellation_during_running_discovery_stays_nonblocking_and_observes_actual_stop() -> TestResult
{
    let mut pipeline = Pipeline::new()?;
    let (fixture, signed) = initialized(&mut pipeline)?;
    let exchanges = vec![
        exchange("2.root.json", None, 404, "Content-Length: 0\r\n", b""),
        complete("timestamp.json", &signed.metadata.timestamp),
    ];
    let (server, policy) =
        pipeline.run(8 * 1024 * 1024, move |context| Running::start(exchanges, &context))?;
    let mut worker = Worker::new(configuration(&fixture, policy))?;
    step(&mut pipeline, &mut worker)?;
    let gate = Arc::new(ClockGate::default());
    let clock = gate.clone();
    pipeline.admit(|client, scope, id| {
        Ok(worker
            .submit(client, scope, id, Moment::from_nanos(2_000_000_000), move || clock.now())?)
    })?;
    gate.entered()?;
    assert!(worker.poll().is_none());
    assert!(worker.pending());
    worker.cancel();
    assert_eq!(worker.progress(), Progress::Retiring);
    assert!(worker.poll().is_none());
    assert!(submit(&mut pipeline, &mut worker).is_err());
    gate.release();
    let observed = pipeline.observe(|| worker.poll())?;
    assert_eq!(observed.progress, Progress::Retiring);
    assert!(matches!(observed.outcome, JobOutcome::Cancelled));
    assert!(observed.failure.is_some());
    server.finish()?;
    retire(&mut pipeline, &mut worker)?;
    assert_unlocked(&mut pipeline, &fixture)?;
    Ok(())
}

#[test]
fn cancellation_after_extraction_releases_live_setup_custody_without_publishing_a_candidate()
-> TestResult {
    let mut pipeline = Pipeline::new()?;
    let (fixture, signed) = initialized(&mut pipeline)?;
    let (mut worker, server) = campaign(&mut pipeline, &fixture, signed, true)?;
    for _ in 0..4 {
        step(&mut pipeline, &mut worker)?;
    }
    server.finish()?;
    assert_eq!(worker.progress(), Progress::Preparing);
    let gate = Arc::new(ClockGate::default());
    let clock = gate.clone();
    let mut samples = 0;
    pipeline.admit(|client, scope, id| {
        Ok(worker.submit(client, scope, id, Moment::from_nanos(2_000_000_000), move || {
            samples += 1;
            if samples == 2 { clock.now() } else { NOW }
        })?)
    })?;
    gate.entered()?;
    let mut check = Pipeline::new()?;
    let root = fixture.installation.path().to_owned();
    check.run(RESERVATION_BYTES, move |context| {
        assert!(InstallationSession::maintain(&root, &context).is_err());
        let store = Arc::new(SelectionStore::open(&root, &context)?);
        assert!(store.load(&context)?.ok_or("Selection absent")?.candidate().is_none());
        drop(InstallationRun::acquire(store, &context)?);
        Ok(())
    })?;
    worker.cancel();
    assert!(worker.pending());
    assert!(worker.poll().is_none());
    gate.release();
    let observed = pipeline.observe(|| worker.poll())?;
    assert!(matches!(observed.outcome, JobOutcome::Cancelled));
    assert!(observed.failure.is_some());
    retire(&mut pipeline, &mut worker)?;
    assert_unlocked(&mut pipeline, &fixture)?;
    assert!(
        !fixture
            .installation
            .path()
            .join(".update-trust")
            .join("offer-v1-application-release-windows-x86_64.json")
            .exists()
    );
    Ok(())
}
