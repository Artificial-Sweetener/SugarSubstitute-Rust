// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Characterize real policy invalidation, cause custody and explicit replacement.
use super::support::*;

pub(super) fn binding(instance: u64) -> Result<ClientBinding, IdentityError> {
    Ok(ClientBinding { run: run(instance)?, connection: connection(instance)? })
}

pub(super) fn death(lease: LeaseKey, status: u32) -> Result<ResourceObservation, IdentityError> {
    Ok(ResourceObservation::ManagedExited {
        resource: lease.resource,
        process: ProcessKey {
            owner: lease.resource.owner,
            id: ProcessId::new(lease.resource.id.get())?,
        },
        status,
        failure: LifecycleFailure::new(
            LifecycleFailureCode::UnexpectedExit,
            Box::new(std::io::Error::other("actual external process cause")),
        ),
    })
}

pub(super) fn settled_cause(
    coordinator: &mut ResourceCoordinator,
    resource: ResourceKey,
) -> Result<LifecycleFailure, Box<dyn Error>> {
    observe(coordinator, ResourceObservation::Settled(resource))?;
    coordinator.take_failure(resource)?.ok_or_else(|| "Missing retained cause".into())
}

pub(super) fn failed_granted() -> Result<(ResourceCoordinator, LeaseKey), Box<dyn Error>> {
    let mut coordinator = ResourceCoordinator::new(owner()?);
    register(&mut coordinator, 1, "outage-first")?;
    let old = lease(&submit(
        &mut coordinator,
        request(1, 1, ResourceCommand::Acquire(target(8188, true)?))?,
    )?)?;
    ready(&mut coordinator, old, 8188, true)?;
    observe(&mut coordinator, death(old, 0)?)?;
    Ok((coordinator, old))
}

#[test]
fn granted_invalidation_counts_unique_runs_not_leases_and_does_not_rewrite_acquire() -> TestResult {
    let mut coordinator = ResourceCoordinator::new(owner()?);
    register(&mut coordinator, 1, "outage-first")?;
    register(&mut coordinator, 2, "outage-second")?;
    let first_request = request(1, 1, ResourceCommand::Acquire(target(8188, true)?))?;
    let first = lease(&submit(&mut coordinator, first_request.clone())?)?;
    ready(&mut coordinator, first, 8188, true)?;
    let same_run = lease(&submit(
        &mut coordinator,
        request(1, 2, ResourceCommand::Acquire(target(8188, true)?))?,
    )?)?;
    let other = lease(&submit(
        &mut coordinator,
        request(2, 1, ResourceCommand::Acquire(target(8188, true)?))?,
    )?)?;
    let transition = observe(&mut coordinator, death(first, 0xc0000005)?)?;
    assert_eq!(transition.invalidations.len(), 3);
    assert!(transition.invalidations.iter().all(|notice| notice.status == 0xc0000005));
    assert!(transition.replies.is_empty());
    assert_eq!(transition.effects, vec![ResourceEffect::StopOwned { resource: first.resource }]);
    for failed in [first, same_run, other] {
        assert_eq!(
            coordinator.affected_clients(
                ClientBinding { run: failed.run, connection: failed.connection },
                failed
            )?,
            2
        );
    }
    assert_eq!(coordinator.snapshot().leases, 0);
    assert_eq!(coordinator.retained_lease_associations(), 3);
    let replay = submit(&mut coordinator, first_request)?;
    assert_eq!(replay.replies[0].outcome, RequestOutcome::Granted(first));
    assert!(replay.effects.is_empty() && replay.invalidations.is_empty());
    assert_eq!(coordinator.snapshot().retained_profiles, 2);
    Ok(())
}

#[test]
fn retry_waits_for_settlement_and_original_cause_transfer_then_replays_exactly() -> TestResult {
    let (mut coordinator, old) = failed_granted()?;
    let retry = request(1, 2, ResourceCommand::RetryDead(old))?;
    assert_eq!(coordinator.invalidated_lease(binding(1)?, old)?, InvalidatedLeaseState::Retiring);
    assert_eq!(submit(&mut coordinator, retry.clone()), Err(LifecycleRejection::WrongPhase));
    assert_eq!(
        coordinator.take_failure(old.resource).map(|_| ()),
        Err(LifecycleRejection::WrongPhase)
    );
    observe(&mut coordinator, ResourceObservation::Settled(old.resource))?;
    assert_eq!(coordinator.invalidated_lease(binding(1)?, old)?, InvalidatedLeaseState::Retiring);
    assert_eq!(submit(&mut coordinator, retry.clone()), Err(LifecycleRejection::WrongPhase));
    let retained = coordinator.take_failure(old.resource)?.ok_or("Missing original cause")?;
    assert!(retained.source().is_some());
    assert_eq!(coordinator.invalidated_lease(binding(1)?, old)?, InvalidatedLeaseState::RetryReady);
    let transition = submit(&mut coordinator, retry.clone())?;
    let replacement = lease(&transition)?;
    assert_eq!(transition.effects.len(), 1);
    assert!(replacement.resource.generation > old.resource.generation);
    assert_eq!(coordinator.invalidated_lease(binding(1)?, old)?, InvalidatedLeaseState::Retrying);
    assert_eq!(coordinator.retained_lease_associations(), 1);
    assert!(submit(&mut coordinator, retry)?.effects.is_empty());
    let changed = request(1, 2, ResourceCommand::Reload(old))?;
    assert_eq!(submit(&mut coordinator, changed), Err(LifecycleRejection::ChangedDuplicate));
    assert_eq!(
        coordinator.adopt_replacement(binding(1)?, old, replacement),
        Err(LifecycleRejection::WrongPhase)
    );
    ready(&mut coordinator, replacement, 8188, true)?;
    assert_eq!(coordinator.invalidated_lease(binding(1)?, old)?, InvalidatedLeaseState::Retrying);
    coordinator.adopt_replacement(binding(1)?, old, replacement)?;
    assert_eq!(coordinator.retained_lease_associations(), 1);
    assert_eq!(
        coordinator.invalidated_lease(binding(1)?, old),
        Err(LifecycleRejection::StaleIdentity)
    );
    assert_eq!(
        submit(&mut coordinator, request(1, 3, ResourceCommand::Release(old))?),
        Err(LifecycleRejection::StaleIdentity)
    );
    assert_eq!(coordinator.resource_phase(replacement.resource), Some(ResourcePhase::Ready));
    Ok(())
}

#[test]
fn rejected_dead_facts_return_owned_causes_and_generic_failure_does_not_authorize_retry()
-> TestResult {
    let mut coordinator = ResourceCoordinator::new(owner()?);
    register(&mut coordinator, 1, "negative")?;
    let current = lease(&submit(
        &mut coordinator,
        request(1, 1, ResourceCommand::Acquire(target(8188, true)?))?,
    )?)?;
    ready(&mut coordinator, current, 8188, true)?;
    let mut wrong = death(current, 0)?;
    if let ResourceObservation::ManagedExited { process, .. } = &mut wrong {
        process.id = ProcessId::new(99)?;
    }
    let rejected = coordinator.observe(wrong).err().ok_or("Wrong process accepted")?;
    assert_eq!(rejected.reason, LifecycleRejection::WrongPhase);
    let ResourceObservation::ManagedExited { failure, .. } = rejected.observation else {
        return Err("Original cause lost".into());
    };
    assert!(failure.source().is_some());
    assert_eq!(coordinator.snapshot().leases, 1);
    let transition = observe(
        &mut coordinator,
        ResourceObservation::Failed {
            resource: current.resource,
            failure: LifecycleFailure::new(
                LifecycleFailureCode::UnexpectedExit,
                Box::new(std::io::Error::other("generic failure")),
            ),
        },
    )?;
    assert!(transition.invalidations.is_empty());
    assert_eq!(
        coordinator.invalidated_lease(binding(1)?, current),
        Err(LifecycleRejection::StaleIdentity)
    );
    assert_eq!(
        submit(&mut coordinator, request(1, 2, ResourceCommand::RetryDead(current))?),
        Err(LifecycleRejection::StaleIdentity)
    );
    Ok(())
}

#[test]
fn attached_and_live_backend_have_no_dead_managed_retry_authority() -> TestResult {
    let mut coordinator = ResourceCoordinator::new(owner()?);
    register(&mut coordinator, 1, "attached")?;
    let attached = lease(&submit(
        &mut coordinator,
        request(1, 1, ResourceCommand::Acquire(target(8188, false)?))?,
    )?)?;
    ready(&mut coordinator, attached, 8188, false)?;
    let rejected =
        coordinator.observe(death(attached, 0)?).err().ok_or("Attached death accepted")?;
    assert_eq!(rejected.reason, LifecycleRejection::WrongPhase);
    assert_eq!(
        submit(&mut coordinator, request(1, 2, ResourceCommand::RetryDead(attached))?),
        Err(LifecycleRejection::StaleIdentity)
    );
    assert_eq!(coordinator.snapshot().leases, 1);
    assert_eq!(coordinator.resource_phase(attached.resource), Some(ResourcePhase::Ready));
    let managed = lease(&submit(
        &mut coordinator,
        request(1, 2, ResourceCommand::Acquire(target(8189, true)?))?,
    )?)?;
    ready(&mut coordinator, managed, 8189, true)?;
    assert_eq!(
        submit(&mut coordinator, request(1, 3, ResourceCommand::RetryDead(managed))?),
        Err(LifecycleRejection::StaleIdentity)
    );
    assert_eq!(coordinator.resource_phase(managed.resource), Some(ResourcePhase::Ready));
    assert_eq!(coordinator.snapshot().leases, 2);
    Ok(())
}

#[test]
fn original_typed_cause_survives_rejected_cleanup_failure_and_explicit_transfer() -> TestResult {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    #[derive(Debug)]
    struct NativeCause(Arc<AtomicUsize>);
    impl std::fmt::Display for NativeCause {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("native cause probe")
        }
    }
    impl Error for NativeCause {}
    impl Drop for NativeCause {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    let dropped = Arc::new(AtomicUsize::new(0));
    let mut coordinator = ResourceCoordinator::new(owner()?);
    register(&mut coordinator, 1, "cause-owner")?;
    let old = lease(&submit(
        &mut coordinator,
        request(1, 1, ResourceCommand::Acquire(target(8188, true)?))?,
    )?)?;
    ready(&mut coordinator, old, 8188, true)?;
    let mut actual = death(old, 4)?;
    if let ResourceObservation::ManagedExited { failure, .. } = &mut actual {
        *failure = LifecycleFailure::new(
            LifecycleFailureCode::UnexpectedExit,
            Box::new(NativeCause(Arc::clone(&dropped))),
        );
    }
    observe(&mut coordinator, actual)?;
    let rejected = coordinator
        .observe(ResourceObservation::Failed {
            resource: old.resource,
            failure: LifecycleFailure::new(
                LifecycleFailureCode::Cleanup,
                Box::new(std::io::Error::other("cleanup still failed")),
            ),
        })
        .err()
        .ok_or("Later failure replaced first")?;
    assert_eq!(rejected.reason, LifecycleRejection::WrongPhase);
    assert_eq!(coordinator.invalidated_lease(binding(1)?, old)?, InvalidatedLeaseState::Retiring);
    assert_eq!(dropped.load(Ordering::SeqCst), 0);
    let transferred = settled_cause(&mut coordinator, old.resource)?;
    assert!(transferred.source().is_some_and(|source| source.is::<NativeCause>()));
    assert_eq!(dropped.load(Ordering::SeqCst), 0);
    assert_eq!(coordinator.invalidated_lease(binding(1)?, old)?, InvalidatedLeaseState::RetryReady);
    drop(coordinator);
    assert_eq!(dropped.load(Ordering::SeqCst), 0);
    drop(transferred);
    assert_eq!(dropped.load(Ordering::SeqCst), 1);
    Ok(())
}

#[test]
fn failed_successor_is_retrying_until_its_own_cleanup_and_cause_transfer() -> TestResult {
    let (mut coordinator, old) = failed_granted()?;
    let first = settled_cause(&mut coordinator, old.resource)?;
    let replacement =
        lease(&submit(&mut coordinator, request(1, 2, ResourceCommand::RetryDead(old))?)?)?;
    let failure = observe(
        &mut coordinator,
        ResourceObservation::Failed {
            resource: replacement.resource,
            failure: LifecycleFailure::new(
                LifecycleFailureCode::Preparation,
                Box::new(std::io::Error::other("successor preparation")),
            ),
        },
    )?;
    assert_eq!(failure.replies[0].outcome, RequestOutcome::Failed);
    assert!(failure.invalidations.is_empty());
    assert_eq!(coordinator.invalidated_lease(binding(1)?, old)?, InvalidatedLeaseState::Retrying);
    assert_eq!(
        submit(&mut coordinator, request(1, 3, ResourceCommand::RetryDead(old))?),
        Err(LifecycleRejection::WrongPhase)
    );
    observe(&mut coordinator, ResourceObservation::Settled(replacement.resource))?;
    assert_eq!(coordinator.invalidated_lease(binding(1)?, old)?, InvalidatedLeaseState::Retrying);
    let second = coordinator.take_failure(replacement.resource)?.ok_or("Lost successor cause")?;
    assert_eq!(coordinator.invalidated_lease(binding(1)?, old)?, InvalidatedLeaseState::RetryReady);
    let next = lease(&submit(&mut coordinator, request(1, 3, ResourceCommand::RetryDead(old))?)?)?;
    assert!(next.resource.generation > replacement.resource.generation);
    assert!(first.source().is_some() && second.source().is_some());
    assert_eq!(coordinator.retained_lease_associations(), 1);
    Ok(())
}
