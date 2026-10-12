// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Require exact original jobs and consumed success before publishing cached authority.
use super::*;
#[test]
fn cached_namespace_requires_actual_consumed_discovery_and_transfers_only_once() -> TestResult {
    let mut h = Harness::new()?;
    assert!(h.owner.take_namespace().is_none());
    let gate = Arc::new(Gate::default());
    *lock(&h.fake.gate) = Some((Stage::Discover, gate.clone()));
    let spec = h.setup_spec()?;
    h.owner.submit_discovery(&h.client, &h.scope, spec)?;
    gate.wait()?;
    assert!(h.owner.take_namespace().is_none());
    assert_eq!(h.owner.snapshot().phase, EntryPhase::Discovering);
    let foreign =
        h.client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
    let probe = h.spec(Moment::from_nanos(20_000_000_000))?;
    let mut job = h.client.submit_blocking(&foreign, probe, |_| Ok::<_, ClaimError>(()))?;
    let completion = h.consume_job(&mut job)?;
    assert!(matches!(completion.outcome, JobOutcome::Success(())));
    drop(completion);
    assert!(h.owner.take_namespace().is_none());
    gate.release();
    assert!(matches!(h.drain()?, Some(EntryObservation::NamespacePrepared(_))));
    let proof = h.owner.take_namespace().ok_or("actual namespace proof absent")?;
    assert_eq!(proof.namespace(), facts()?.namespace());
    assert_eq!(proof.installation_identity(), [6; 24]);
    assert_eq!(proof.execution_owner(), h.scope.owner_id());
    assert_eq!(proof.original_preparation(), h.owner.budget);
    assert!(h.owner.take_namespace().is_none());
    proof.validate_live()?;
    h.owner.close_admission();
    assert!(proof.validate_live().is_err());
    assert!(matches!(h.close(Moment::from_nanos(40_000_000_000))?, EntryObservation::Retired));
    Ok(())
}
#[test]
fn invalid_specification_rejects_before_any_external_discovery() -> TestResult {
    let mut h = Harness::new()?;
    let mut spec = h.setup_spec()?;
    spec.reservation_bytes -= 1;
    assert!(matches!(
        h.owner.submit_discovery(&h.client, &h.scope, spec),
        Err(EntryAdmissionError::Specification)
    ));
    let mut spec = h.setup_spec()?;
    spec.deadline = Some(Moment::from_nanos(h.owner.budget.until().as_nanos() + 1));
    assert!(matches!(
        h.owner.submit_discovery(&h.client, &h.scope, spec),
        Err(EntryAdmissionError::Closed)
    ));
    assert_eq!(h.fake.calls[0].load(Ordering::Acquire), 0);
    assert!(matches!(h.close(Moment::from_nanos(40_000_000_000))?, EntryObservation::Retired));
    Ok(())
}
#[test]
fn election_requires_consumed_discovery_and_exact_execution_owner() -> TestResult {
    let mut h = Harness::new()?;
    let spec = h.setup_spec()?;
    assert!(matches!(
        h.owner.submit_election(&h.client, &h.scope, spec),
        Err(EntryAdmissionError::Busy)
    ));
    h.discover()?;
    let other = ExecutionOwner::new(h.clock.clone(), Arc::new(Wake::default()))?;
    let client = other.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let spec = h.setup_spec()?;
    assert!(matches!(
        h.owner.submit_election(&client, &scope, spec),
        Err(EntryAdmissionError::Identity)
    ));
    assert_eq!(h.fake.calls[1].load(Ordering::Acquire), 0);
    assert!(matches!(h.elect()?, EntryObservation::ElectionHeld));
    let mut claims = WindowsResourceClaims::new()?;
    h.owner.prepare_claims(&mut claims)?;
    assert!(h.owner.prepare_claims(&mut claims).is_err());
    h.owner.close_admission();
    let mut fresh = WindowsResourceClaims::new()?;
    assert!(matches!(h.owner.prepare_claims(&mut fresh), Err(EntryAdmissionError::Closed)));
    assert!(matches!(h.close(Moment::from_nanos(40_000_000_000))?, EntryObservation::Retired));
    Ok(())
}
#[test]
fn oversized_completion_batch_is_rejected_without_consuming_current_work() -> TestResult {
    let mut h = Harness::new()?;
    let gate = Arc::new(Gate::default());
    *lock(&h.fake.gate) = Some((Stage::Discover, gate.clone()));
    let spec = h.setup_spec()?;
    h.owner.submit_discovery(&h.client, &h.scope, spec)?;
    gate.wait()?;
    let notice = sugarsubstitute_contracts::execution::CompletionNotification {
        scope_id: h.scope.id(),
        job_id: spec.id,
        kind: sugarsubstitute_contracts::execution::TerminalKind::Success,
        replacement: None,
    };
    let batch = sugarsubstitute_contracts::execution::CompletionBatch {
        notifications: vec![notice; 33],
        remaining: 0,
    };
    assert!(h.owner.consume(&batch).is_err());
    assert!(h.owner.pending.is_some());
    assert!(h.owner.take_namespace().is_none());
    gate.release();
    assert!(matches!(h.drain()?, Some(EntryObservation::NamespacePrepared(_))));
    assert!(matches!(h.close(Moment::from_nanos(40_000_000_000))?, EntryObservation::Retired));
    Ok(())
}
