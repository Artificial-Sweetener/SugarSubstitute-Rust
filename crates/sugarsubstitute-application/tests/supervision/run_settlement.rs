// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep policy settlement pending only for the retiring run's actual current cleanup.
use super::support::*;

#[test]
fn last_client_retirement_waits_for_actual_backend_settlement_without_releasing_profile()
-> TestResult {
    let mut policy = ResourceCoordinator::new(owner()?);
    let profile = ProfileId::new("exclusive".into())?;
    assert!(!policy.profile_successor_waiting(&profile, run(1)?)?);
    register(&mut policy, 1, "exclusive")?;
    assert!(policy.profile_successor_waiting(&profile, run(1)?)?);
    let held = lease(&submit(
        &mut policy,
        request(1, 1, ResourceCommand::Acquire(target(8188, true)?))?,
    )?)?;
    ready(&mut policy, held, 8188, true)?;
    let released = submit(&mut policy, request(1, 2, ResourceCommand::Release(held))?)?;
    assert_eq!(released.replies[0].outcome, RequestOutcome::Released);
    assert_eq!(released.effects, vec![ResourceEffect::StopOwned { resource: held.resource }]);
    assert!(policy.disconnect_run(run(1)?, connection(1)?)?.effects.is_empty());
    assert_eq!(policy.run_settlement(run(1)?)?, RunSettlementStatus::PendingBackendCleanup);
    assert_eq!(policy.settle_run(run(1)?), Err(LifecycleRejection::WrongPhase));
    assert_eq!(policy.snapshot().retained_profiles, 1);
    assert!(policy.profile_successor_waiting(&profile, run(1)?)?);
    assert!(register(&mut policy, 2, "exclusive").is_err());
    // External adapter observation is supplied through the genuine policy transition.
    observe(&mut policy, ResourceObservation::Settled(held.resource))?;
    assert_eq!(policy.run_settlement(run(1)?)?, RunSettlementStatus::Ready);
    assert_eq!(policy.snapshot().retained_profiles, 1);
    assert!(policy.profile_successor_waiting(&profile, run(1)?)?);
    policy.settle_run(run(1)?)?;
    assert_eq!(policy.snapshot().retained_profiles, 0);
    assert!(!policy.profile_successor_waiting(&profile, run(1)?)?);
    // An exact repeated close neither removes the retained identity nor creates effects.
    let settled = policy.snapshot();
    policy.settle_run(run(1)?)?;
    assert_eq!(policy.snapshot(), settled);
    register(&mut policy, 2, "exclusive")?;
    assert_eq!(
        policy.profile_successor_waiting(&profile, run(1)?),
        Err(LifecycleRejection::ProfileContention)
    );
    Ok(())
}

/// Advisory exclusion never treats another run or foreign epoch as the bound predecessor.
#[test]
fn profile_successor_advisory_correlates_exact_occupant_and_preserves_enrollment() -> TestResult {
    let mut policy = ResourceCoordinator::new(owner()?);
    let profile = ProfileId::new("exclusive".into())?;
    register(&mut policy, 1, "exclusive")?;
    let before = policy.snapshot();
    assert!(policy.profile_successor_waiting(&profile, run(1)?)?);
    assert!(!policy.profile_successor_waiting(&ProfileId::new("distinct".into())?, run(1)?)?);
    assert_eq!(
        policy.profile_successor_waiting(&profile, run(2)?),
        Err(LifecycleRejection::ProfileContention)
    );
    let stale = RunKey { generation: RunGeneration::new(2)?, ..run(1)? };
    assert_eq!(
        policy.profile_successor_waiting(&profile, stale),
        Err(LifecycleRejection::ProfileContention)
    );
    let mut foreign = run(1)?;
    foreign.owner.epoch = CoordinatorEpoch::new([3; 16])?;
    assert_eq!(
        policy.profile_successor_waiting(&profile, foreign),
        Err(LifecycleRejection::ForeignOwner)
    );
    assert!(register(&mut policy, 2, "exclusive").is_err());
    assert_eq!(policy.snapshot(), before);
    Ok(())
}

#[test]
fn settlement_status_rejects_connected_live_lease_foreign_and_stale_runs() -> TestResult {
    let mut policy = ResourceCoordinator::new(owner()?);
    register(&mut policy, 1, "first")?;
    assert_eq!(policy.run_settlement(run(1)?), Err(LifecycleRejection::WrongPhase));
    let held = lease(&submit(
        &mut policy,
        request(1, 1, ResourceCommand::Acquire(target(8188, true)?))?,
    )?)?;
    ready(&mut policy, held, 8188, true)?;
    let mut foreign = run(1)?;
    foreign.owner.epoch = CoordinatorEpoch::new([3; 16])?;
    let stale = RunKey { generation: RunGeneration::new(2)?, ..run(1)? };
    for (candidate, rejection) in [
        (run(1)?, LifecycleRejection::WrongPhase),
        (foreign, LifecycleRejection::ForeignOwner),
        (stale, LifecycleRejection::StaleIdentity),
    ] {
        let retained = policy.snapshot();
        assert_eq!(policy.run_settlement(candidate), Err(rejection));
        assert_eq!(policy.settle_run(candidate), Err(rejection));
        assert_eq!(policy.snapshot(), retained);
    }
    assert_eq!(policy.snapshot().leases, 1);
    assert_eq!(policy.resource_phase(held.resource), Some(ResourcePhase::Ready));
    Ok(())
}

#[test]
fn run_settlement_ignores_other_clients_live_lease_and_unrelated_cleanup() -> TestResult {
    let mut policy = ResourceCoordinator::new(owner()?);
    for (id, profile) in [(1, "first"), (2, "second"), (3, "third")] {
        register(&mut policy, id, profile)?;
    }
    let first = lease(&submit(
        &mut policy,
        request(1, 1, ResourceCommand::Acquire(target(8188, true)?))?,
    )?)?;
    let second = lease(&submit(
        &mut policy,
        request(2, 1, ResourceCommand::Acquire(target(8188, true)?))?,
    )?)?;
    assert_eq!(first.resource, second.resource);
    ready(&mut policy, first, 8188, true)?;
    let unrelated = lease(&submit(
        &mut policy,
        request(3, 1, ResourceCommand::Acquire(target(8189, true)?))?,
    )?)?;
    ready(&mut policy, unrelated, 8189, true)?;
    assert!(
        submit(&mut policy, request(1, 2, ResourceCommand::Release(first))?)?.effects.is_empty()
    );
    policy.disconnect_run(run(1)?, connection(1)?)?;
    let stopped = policy.disconnect_run(run(3)?, connection(3)?)?;
    assert_eq!(stopped.effects, vec![ResourceEffect::StopOwned { resource: unrelated.resource }]);
    assert_eq!(policy.run_settlement(run(3)?)?, RunSettlementStatus::PendingBackendCleanup);
    assert_eq!(policy.run_settlement(run(1)?)?, RunSettlementStatus::Ready);
    policy.settle_run(run(1)?)?;
    assert_eq!(policy.snapshot().leases, 1);
    assert_eq!(policy.snapshot().resources, 2);
    assert_eq!(policy.resource_phase(second.resource), Some(ResourcePhase::Ready));
    assert_eq!(policy.resource_phase(unrelated.resource), Some(ResourcePhase::Stopping));
    assert_eq!(policy.run_settlement(run(2)?), Err(LifecycleRejection::WrongPhase));
    Ok(())
}

#[test]
fn pending_uncertain_cleanup_keeps_original_failure_through_actual_settlement() -> TestResult {
    let mut policy = ResourceCoordinator::new(owner()?);
    register(&mut policy, 1, "first")?;
    let held = lease(&submit(
        &mut policy,
        request(1, 1, ResourceCommand::Acquire(target(8188, true)?))?,
    )?)?;
    let cause: Box<dyn Error + Send + Sync> =
        Box::new(std::io::Error::other("original cleanup cause"));
    let original = std::ptr::from_ref(cause.as_ref());
    observe(
        &mut policy,
        ResourceObservation::Failed {
            resource: held.resource,
            failure: LifecycleFailure::new(LifecycleFailureCode::Preparation, cause),
        },
    )?;
    policy.disconnect_run(run(1)?, connection(1)?)?;
    assert_eq!(policy.resource_phase(held.resource), Some(ResourcePhase::Uncertain));
    assert_eq!(policy.run_settlement(run(1)?)?, RunSettlementStatus::PendingBackendCleanup);
    assert_eq!(policy.settle_run(run(1)?), Err(LifecycleRejection::WrongPhase));
    let retained =
        policy.failure(held.resource).and_then(Error::source).ok_or("Original cause absent")?;
    assert!(std::ptr::eq(retained, original as *const dyn Error));
    observe(&mut policy, ResourceObservation::Settled(held.resource))?;
    assert_eq!(policy.run_settlement(run(1)?)?, RunSettlementStatus::Ready);
    policy.settle_run(run(1)?)?;
    let failure = policy.take_failure(held.resource)?.ok_or("Original failure lost")?;
    assert!(std::ptr::eq(
        failure.source().ok_or("Original source lost")?,
        original as *const dyn Error
    ));
    Ok(())
}
