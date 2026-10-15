// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve the complete same-origin cohort without a spare association credit.
use super::{
    outage::{binding, death, failed_granted, settled_cause},
    support::*,
};

#[test]
fn one_user_retry_retains_other_main_before_ack_and_only_last_disconnect_stops() -> TestResult {
    let mut coordinator = ResourceCoordinator::new(owner()?);
    register(&mut coordinator, 1, "requesting-main")?;
    register(&mut coordinator, 2, "automatically-reconnecting-main")?;
    let a = lease(&submit(
        &mut coordinator,
        request(1, 1, ResourceCommand::Acquire(target(8188, true)?))?,
    )?)?;
    let b = lease(&submit(
        &mut coordinator,
        request(2, 1, ResourceCommand::Acquire(target(8188, true)?))?,
    )?)?;
    ready(&mut coordinator, a, 8188, true)?;
    observe(&mut coordinator, death(a, 9)?)?;
    let original = settled_cause(&mut coordinator, a.resource)?;
    let transition = submit(&mut coordinator, request(1, 2, ResourceCommand::RetryDead(a))?)?;
    let new_a = lease(&transition)?;
    let new_b =
        coordinator.replacement_lease(binding(2)?, b)?.ok_or("Other Main interest missing")?;
    assert_ne!(new_b.id, b.id);
    assert_eq!(new_b.run, b.run);
    assert_eq!(new_b.connection, b.connection);
    assert_eq!(new_b.resource, new_a.resource);
    assert_eq!(coordinator.snapshot().leases, 2);
    assert_eq!(coordinator.retained_lease_associations(), 2);
    assert_eq!(coordinator.invalidated_lease(binding(2)?, b)?, InvalidatedLeaseState::Retrying);
    assert_eq!(transition.effects.len(), 1);
    // The requester closes before the other Main receives Ready or acknowledges
    // its new binding. Its ordinary same-origin reconnect still has live interest.
    let first_close = coordinator.disconnect_run(run(1)?, connection(1)?)?;
    assert!(first_close.effects.is_empty());
    assert_eq!(coordinator.snapshot().leases, 1);
    assert_eq!(coordinator.retained_lease_associations(), 1);
    observe(&mut coordinator, ResourceObservation::ClaimAcquired(new_b.resource))?;
    observe(
        &mut coordinator,
        ResourceObservation::OwnedStarted {
            resource: new_b.resource,
            process: ProcessKey { owner: owner()?, id: ProcessId::new(new_b.resource.id.get())? },
        },
    )?;
    let ready = observe(
        &mut coordinator,
        ResourceObservation::Ready {
            resource: new_b.resource,
            origin: LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?,
        },
    )?;
    assert_eq!(
        ready.replacements,
        vec![GrantedResourceReplacement { previous: b, current: new_b }]
    );
    assert!(ready.replies.is_empty());
    assert_eq!(
        coordinator.adopt_replacement(binding(2)?, b, new_a),
        Err(LifecycleRejection::WrongPhase)
    );
    coordinator.adopt_replacement(binding(2)?, b, new_b)?;
    assert_eq!(coordinator.snapshot().retained_profiles, 2);
    let last_close = coordinator.disconnect_run(run(2)?, connection(2)?)?;
    assert_eq!(last_close.effects, vec![ResourceEffect::StopOwned { resource: new_b.resource }]);
    assert_eq!(coordinator.resource_phase(new_b.resource), Some(ResourcePhase::Stopping));
    assert_eq!(coordinator.retained_lease_associations(), 0);
    assert!(original.source().is_some());
    Ok(())
}

#[test]
fn all_256_credits_transfer_to_fresh_cohort_without_a_257th_slot() -> TestResult {
    let mut coordinator = ResourceCoordinator::new(owner()?);
    let mut old = Vec::new();
    for instance in 1..=MAX_RUNS as u64 {
        register(&mut coordinator, instance, &format!("cohort-{instance}"))?;
        for id in 1..=4 {
            old.push(lease(&submit(
                &mut coordinator,
                request(instance, id, ResourceCommand::Acquire(target(8188, true)?))?,
            )?)?);
        }
    }
    let first = *old.first().ok_or("Missing cohort")?;
    ready(&mut coordinator, first, 8188, true)?;
    assert_eq!(coordinator.retained_lease_associations(), MAX_LEASES);
    let dead = observe(&mut coordinator, death(first, u32::MAX)?)?;
    assert_eq!(dead.invalidations.len(), MAX_LEASES);
    assert!(dead.replies.is_empty());
    for failed in &old {
        assert_eq!(
            coordinator.affected_clients(
                ClientBinding { run: failed.run, connection: failed.connection },
                *failed
            )?,
            MAX_RUNS as u16
        );
    }
    let original = settled_cause(&mut coordinator, first.resource)?;
    // A generic Acquire cannot bypass the user-authorized dead-generation route.
    assert_eq!(
        submit(&mut coordinator, request(1, 5, ResourceCommand::Acquire(target(8188, true)?))?),
        Err(LifecycleRejection::WrongPhase)
    );
    let retried = submit(&mut coordinator, request(1, 5, ResourceCommand::RetryDead(first))?)?;
    let replacement = lease(&retried)?;
    assert_eq!(retried.effects.len(), 1);
    assert_eq!(coordinator.retained_lease_associations(), MAX_LEASES);
    assert_eq!(coordinator.snapshot().leases, MAX_LEASES);
    assert_eq!(coordinator.snapshot().outstanding_requests, 1);
    assert_eq!(
        submit(&mut coordinator, request(1, 6, ResourceCommand::Acquire(target(8189, true)?))?),
        Err(LifecycleRejection::Overloaded)
    );
    let mut new_ids = std::collections::BTreeSet::new();
    for failed in &old {
        let next = coordinator
            .replacement_lease(
                ClientBinding { run: failed.run, connection: failed.connection },
                *failed,
            )?
            .ok_or("Lost cohort interest")?;
        assert_eq!(next.resource, replacement.resource);
        assert_eq!(next.run, failed.run);
        assert_eq!(next.connection, failed.connection);
        assert!(next.id > failed.id);
        assert!(new_ids.insert(next.id));
    }
    ready(&mut coordinator, replacement, 8188, true)?;
    for instance in 1..MAX_RUNS as u64 {
        assert!(
            coordinator.disconnect_run(run(instance)?, connection(instance)?)?.effects.is_empty()
        );
    }
    let last = coordinator.disconnect_run(run(MAX_RUNS as u64)?, connection(MAX_RUNS as u64)?)?;
    assert_eq!(last.effects, vec![ResourceEffect::StopOwned { resource: replacement.resource }]);
    assert_eq!(coordinator.retained_lease_associations(), 0);
    assert_eq!(coordinator.snapshot().retained_profiles, MAX_RUNS);
    assert!(original.source().is_some());
    Ok(())
}

#[test]
fn pending_managed_exit_finishes_request_without_inventing_a_granted_invalidation() -> TestResult {
    let mut coordinator = ResourceCoordinator::new(owner()?);
    register(&mut coordinator, 1, "pending")?;
    let pending = lease(&submit(
        &mut coordinator,
        request(1, 1, ResourceCommand::Acquire(target(8188, true)?))?,
    )?)?;
    observe(&mut coordinator, ResourceObservation::ClaimAcquired(pending.resource))?;
    observe(
        &mut coordinator,
        ResourceObservation::OwnedStarted {
            resource: pending.resource,
            process: ProcessKey { owner: owner()?, id: ProcessId::new(pending.resource.id.get())? },
        },
    )?;
    let dead = observe(&mut coordinator, death(pending, 0)?)?;
    assert!(dead.invalidations.is_empty() && dead.replacements.is_empty());
    assert_eq!(dead.replies[0].outcome, RequestOutcome::Failed);
    assert_eq!(
        coordinator.invalidated_lease(binding(1)?, pending),
        Err(LifecycleRejection::StaleIdentity)
    );
    assert_eq!(coordinator.retained_lease_associations(), 0);
    Ok(())
}

#[test]
fn stale_foreign_and_changed_connection_cannot_retry_or_adopt() -> TestResult {
    let (mut coordinator, old) = failed_granted()?;
    let original = settled_cause(&mut coordinator, old.resource)?;
    let wrong = ClientBinding { connection: connection(99)?, ..binding(1)? };
    assert_eq!(coordinator.invalidated_lease(wrong, old), Err(LifecycleRejection::StaleIdentity));
    let mut foreign = binding(1)?;
    foreign.run.owner.namespace = NamespaceId::new([99; 32]);
    assert_eq!(coordinator.invalidated_lease(foreign, old), Err(LifecycleRejection::ForeignOwner));
    let stale = LeaseKey { connection: connection(99)?, ..old };
    assert_eq!(
        submit(&mut coordinator, request(1, 2, ResourceCommand::RetryDead(stale))?),
        Err(LifecycleRejection::StaleIdentity)
    );
    assert_eq!(coordinator.retained_lease_associations(), 1);
    coordinator.disconnect_run(run(1)?, connection(1)?)?;
    assert_eq!(coordinator.retained_lease_associations(), 0);
    assert_eq!(
        coordinator.invalidated_lease(binding(1)?, old),
        Err(LifecycleRejection::StaleIdentity)
    );
    assert_eq!(coordinator.snapshot().retained_profiles, 1);
    assert!(original.source().is_some());
    Ok(())
}

#[test]
fn repeated_successor_death_reuses_slot_and_rejects_old_generation_facts() -> TestResult {
    let (mut coordinator, old) = failed_granted()?;
    let first = settled_cause(&mut coordinator, old.resource)?;
    let replacement =
        lease(&submit(&mut coordinator, request(1, 2, ResourceCommand::RetryDead(old))?)?)?;
    ready(&mut coordinator, replacement, 8188, true)?;
    let second = observe(&mut coordinator, death(replacement, 7)?)?;
    assert_eq!(
        second.invalidations,
        vec![GrantedResourceInvalidation { lease: replacement, status: 7 }]
    );
    assert_eq!(coordinator.retained_lease_associations(), 1);
    assert_eq!(
        coordinator.invalidated_lease(binding(1)?, old),
        Err(LifecycleRejection::StaleIdentity)
    );
    assert_eq!(
        coordinator.invalidated_lease(binding(1)?, replacement)?,
        InvalidatedLeaseState::Retiring
    );
    assert_eq!(observe(&mut coordinator, death(old, 8)?), Err(LifecycleRejection::StaleIdentity));
    let second_cause = settled_cause(&mut coordinator, replacement.resource)?;
    assert!(first.source().is_some() && second_cause.source().is_some());
    let next =
        lease(&submit(&mut coordinator, request(1, 3, ResourceCommand::RetryDead(replacement))?)?)?;
    assert!(next.resource.generation > replacement.resource.generation);
    assert_eq!(coordinator.retained_lease_associations(), 1);
    Ok(())
}

#[test]
fn retained_outage_keeps_exact_target_through_unrelated_settled_record_eviction() -> TestResult {
    let (mut coordinator, old) = failed_granted()?;
    let original = settled_cause(&mut coordinator, old.resource)?;
    register(&mut coordinator, 2, "unrelated-records")?;
    for index in 0..MAX_RESOURCES as u64 - 1 {
        let port = 9000 + u16::try_from(index)?;
        let acquired = lease(&submit(
            &mut coordinator,
            request(2, index * 2 + 1, ResourceCommand::Acquire(target(port, false)?))?,
        )?)?;
        submit(&mut coordinator, request(2, index * 2 + 2, ResourceCommand::Release(acquired))?)?;
        observe(&mut coordinator, ResourceObservation::Settled(acquired.resource))?;
    }
    let unrelated = lease(&submit(
        &mut coordinator,
        request(2, 127, ResourceCommand::Acquire(target(10000, false)?))?,
    )?)?;
    assert_ne!(unrelated.resource.id, old.resource.id);
    assert_eq!(coordinator.resource_phase(old.resource), Some(ResourcePhase::Settled));
    let retry = submit(&mut coordinator, request(1, 2, ResourceCommand::RetryDead(old))?)?;
    let replacement = lease(&retry)?;
    assert_eq!(replacement.resource.id, old.resource.id);
    assert!(
        matches!(retry.effects.first(), Some(ResourceEffect::Prepare { target: current, .. }) if *current == target(8188, true)?)
    );
    assert!(original.source().is_some());
    Ok(())
}
