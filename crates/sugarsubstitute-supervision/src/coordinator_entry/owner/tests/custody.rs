// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep actual preparation custody across cancellation, late returns and failed close.
use super::*;
#[test]
fn cancelled_discovery_preserves_actual_storage_without_namespace_proof() -> TestResult {
    let mut h = Harness::new()?;
    let gate = Arc::new(Gate::default());
    *lock(&h.fake.gate) = Some((Stage::Discover, gate.clone()));
    let spec = h.setup_spec()?;
    h.owner.submit_discovery(&h.client, &h.scope, spec)?;
    gate.wait()?;
    h.owner.close_admission();
    assert_eq!(h.scope.snapshot().reserved_bytes, ENTRY_PREPARATION_BYTES);
    gate.release();
    assert!(matches!(h.drain()?, Some(EntryObservation::Stopped(_))));
    assert!(h.owner.take_namespace().is_none());
    assert!(lock(&h.owner.lifetime.state).memory.is_some());
    assert!(!h.owner.lifetime.retired.load(Ordering::Acquire));
    assert!(matches!(h.close(Moment::from_nanos(40_000_000_000))?, EntryObservation::Retired));
    assert!(lock(&h.owner.lifetime.state).memory.is_none());
    Ok(())
}
#[test]
fn late_actual_discovery_and_election_never_publish_preparation_proof() -> TestResult {
    for stage in [Stage::Discover, Stage::Elect] {
        let mut h = Harness::new()?;
        if stage == Stage::Elect {
            h.discover()?;
        }
        *lock(&h.fake.expire_after) = Some(stage);
        let event = if stage == Stage::Discover { h.discover()? } else { h.elect()? };
        assert!(matches!(event, EntryObservation::Stopped(_)));
        assert!(h.owner.election.is_none());
        assert!(h.owner.take_namespace().is_none());
        assert!(lock(&h.owner.lifetime.state).memory.is_some());
        assert!(matches!(h.close(Moment::from_nanos(50_000_000_000))?, EntryObservation::Retired));
    }
    Ok(())
}
#[test]
fn failed_close_preserves_election_and_original_cleanup_deadline_across_retries() -> TestResult {
    let mut h = Harness::new()?;
    h.discover()?;
    h.elect()?;
    *lock(&h.fake.failure) = Some(Stage::Close);
    let until = Moment::from_nanos(40_000_000_000);
    assert!(
        matches!(h.close(until)?,EntryObservation::Failed(ref cause) if cause.0.operation()=="entry_test_native_failure")
    );
    assert!(h.fake.locked.load(Ordering::Acquire));
    assert!(!h.owner.lifetime.retired.load(Ordering::Acquire));
    assert!(lock(&h.owner.lifetime.state).memory.is_some());
    h.clock.0.store(5_000_000_000, Ordering::Release);
    let spec = h.spec(Moment::from_nanos(45_000_000_000))?;
    assert!(matches!(
        h.owner.submit_close(&h.client, &h.scope, spec),
        Err(EntryAdmissionError::Specification)
    ));
    assert_eq!(h.fake.calls[2].load(Ordering::Acquire), 1);
    assert!(matches!(h.close(until)?, EntryObservation::Retired));
    assert_eq!(*lock(&h.fake.close_deadlines), [until, until]);
    assert!(!h.fake.locked.load(Ordering::Acquire));
    Ok(())
}
#[test]
fn actual_retirement_after_late_terminal_is_reported_separately_once() -> TestResult {
    let mut h = Harness::new()?;
    h.discover()?;
    h.elect()?;
    *lock(&h.fake.expire_after) = Some(Stage::Close);
    assert!(matches!(h.close(Moment::from_nanos(40_000_000_000))?, EntryObservation::Stopped(_)));
    let empty = h.client.drain_completions(NonZeroUsize::new(32).ok_or("batch bound")?)?;
    assert!(matches!(h.owner.consume(&empty)?, Some(EntryObservation::Retired)));
    assert!(h.owner.consume(&empty)?.is_none());
    assert_eq!(h.owner.snapshot().phase, EntryPhase::Settled);
    assert!(lock(&h.owner.lifetime.state).memory.is_none());
    assert!(!h.fake.locked.load(Ordering::Acquire));
    Ok(())
}
#[test]
fn rejected_cleanup_admission_latches_first_deadline_before_rejection() -> TestResult {
    let mut h = Harness::new()?;
    h.discover()?;
    let cancelled =
        h.client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
    cancelled.cancel();
    let until = Moment::from_nanos(40_000_000_000);
    let spec = h.spec(until)?;
    assert!(matches!(
        h.owner.submit_close(&h.client, &cancelled, spec),
        Err(EntryAdmissionError::Execution(_))
    ));
    let spec = h.spec(Moment::from_nanos(45_000_000_000))?;
    assert!(matches!(
        h.owner.submit_close(&h.client, &h.scope, spec),
        Err(EntryAdmissionError::Specification)
    ));
    assert_eq!(h.fake.calls[2].load(Ordering::Acquire), 0);
    assert!(matches!(h.close(until)?, EntryObservation::Retired));
    assert_eq!(*lock(&h.fake.close_deadlines), [until]);
    Ok(())
}

#[test]
fn closed_input_custody_survives_running_discovery_until_actual_return_and_retirement() -> TestResult
{
    let mut h = Harness::new()?;
    let paths = Arc::downgrade(h.owner.paths.as_ref().ok_or("missing inert paths")?);
    let gate = Arc::new(Gate::default());
    *lock(&h.fake.gate) = Some((Stage::Discover, gate.clone()));
    let spec = h.setup_spec()?;
    h.owner.submit_discovery(&h.client, &h.scope, spec)?;
    gate.wait()?;
    let memory = {
        let retained = paths.upgrade().ok_or("running paths were released")?;
        Arc::downgrade(retained.preparation_memory.get().ok_or("missing preparation claim")?)
    };
    h.owner.close_admission();
    assert!(h.owner.request.is_none());
    assert!(h.owner.paths.is_none());
    assert!(paths.upgrade().is_some());
    assert!(memory.upgrade().is_some());
    assert_eq!(h.scope.snapshot().reserved_bytes, ENTRY_PREPARATION_BYTES);
    gate.release();
    assert!(matches!(h.drain()?, Some(EntryObservation::Stopped(_))));
    assert!(paths.upgrade().is_none());
    assert!(memory.upgrade().is_some());
    assert!(h.owner.take_namespace().is_none());
    *lock(&h.fake.failure) = Some(Stage::Close);
    let until = Moment::from_nanos(40_000_000_000);
    assert!(matches!(h.close(until)?, EntryObservation::Failed(_)));
    assert!(memory.upgrade().is_some());
    assert!(matches!(h.close(until)?, EntryObservation::Retired));
    assert!(memory.upgrade().is_none());
    h.join_execution_after_retirement()?;
    assert_eq!(h.scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn unadmitted_input_close_retires_inert_allocations_without_native_effects() -> TestResult {
    let mut h = Harness::new()?;
    let paths = Arc::downgrade(h.owner.paths.as_ref().ok_or("missing inert paths")?);
    h.owner.close_admission();
    h.owner.close_admission();
    assert!(h.owner.paths.is_none());
    assert!(h.owner.request.is_none());
    assert!(paths.upgrade().is_none());
    assert_eq!(h.scope.snapshot().reserved_bytes, 0);
    assert!(h.fake.calls.iter().all(|calls| calls.load(Ordering::Acquire) == 0));
    Ok(())
}
