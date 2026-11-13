// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Inspect genuine policy capacities before retaining an input or native observation.
use super::{
    outage::{binding, death, failed_granted, settled_cause},
    support::*,
};

fn managed_argument_capacity(capacity: usize) -> Result<BackendTarget, Box<dyn Error>> {
    let BackendTarget::Managed(base) = target(8188, true)? else {
        return Err("Managed test target missing".into());
    };
    let mut argument = String::with_capacity(capacity);
    argument.push_str("--actual");
    Ok(BackendTarget::Managed(LaunchConfiguration::new(
        base.root().to_path_buf(),
        base.interpreter().to_path_buf(),
        vec![argument],
        base.port().get(),
    )?))
}

#[test]
fn actual_argument_capacity_can_refuse_before_any_policy_mutation() -> TestResult {
    let mut policy = ResourceCoordinator::new(owner()?);
    register(&mut policy, 1, "actual-capacity")?;
    let request =
        request(1, 1, ResourceCommand::Acquire(managed_argument_capacity(2 * 1024 * 1024)?))?;
    let ResourceCommand::Acquire(BackendTarget::Managed(config)) = &request.command else {
        return Err("Managed request missing".into());
    };
    let actual = config.allocated_bytes().ok_or("Capacity arithmetic overflow")?;
    let before = policy.snapshot();
    let requirement = policy.storage_preflight(ResourceStorageInput::Request(&request))?;
    assert!(requirement.projected_bytes() >= u64::try_from(actual)? * 3);
    assert_eq!(
        requirement.check_allocation_partition(4 * 1024 * 1024),
        Err(LifecycleRejection::Overloaded)
    );
    assert_eq!(policy.snapshot(), before);
    assert_eq!(policy.snapshot().leases, 0);
    assert!(
        policy
            .failure(ResourceKey {
                owner: owner()?,
                id: ResourceId::new(1)?,
                generation: ResourceGeneration::new(1)?,
            })
            .is_none()
    );
    // Refusal borrows the original input; its exact source capacity remains owned.
    assert_eq!(config.allocated_bytes(), Some(actual));
    Ok(())
}

#[test]
fn retained_history_and_duplicate_preflight_charge_real_payloads_without_reexecution() -> TestResult
{
    let mut policy = ResourceCoordinator::new(owner()?);
    register(&mut policy, 1, "history-capacity")?;
    let acquire = request(1, 1, ResourceCommand::Acquire(target(8188, true)?))?;
    let anchor = lease(&submit(&mut policy, acquire.clone())?)?;
    ready(&mut policy, anchor, 8188, true)?;
    let initial = policy.storage_preflight(ResourceStorageInput::Current)?;
    for index in 0..32 {
        let acquired = lease(&submit(
            &mut policy,
            request(1, index * 2 + 2, ResourceCommand::Acquire(target(8188, true)?))?,
        )?)?;
        submit(&mut policy, request(1, index * 2 + 3, ResourceCommand::Release(acquired))?)?;
    }
    let retained = policy.storage_preflight(ResourceStorageInput::Current)?;
    assert!(retained.retained_bytes() > initial.retained_bytes());
    let repeated = request(1, 65, ResourceCommand::Release(anchor))?;
    // ID65 is recorded. Sizing allocates no new retention for a duplicate or
    // changed duplicate; the real request owner still checks its exact command.
    let duplicate = policy.storage_preflight(ResourceStorageInput::Request(&repeated))?;
    assert_eq!(duplicate.projected_bytes(), 0);
    assert_eq!(duplicate.retained_bytes(), retained.retained_bytes());
    assert_eq!(policy.snapshot().leases, 1);
    assert_eq!(retained.total_bytes(), retained.fixed_bytes() + retained.allocation_bytes());
    assert_eq!(
        retained.allocation_bytes(),
        retained.retained_bytes() + retained.projected_bytes() + retained.transient_bytes()
    );
    Ok(())
}

#[test]
fn exact_outage_resource_queries_keep_original_and_successor_until_adoption() -> TestResult {
    let (mut policy, old) = failed_granted()?;
    assert!(policy.retains_outage(old.resource));
    assert!(policy.retains_outage_resource(old.resource.id));
    let wrong_generation = ResourceKey {
        generation: ResourceGeneration::new(old.resource.generation.get() + 1)?,
        ..old.resource
    };
    assert!(!policy.retains_outage(wrong_generation));
    let original = settled_cause(&mut policy, old.resource)?;
    let retry = request(1, 2, ResourceCommand::RetryDead(old))?;
    let snapshot = policy.snapshot();
    let projected = policy.storage_preflight(ResourceStorageInput::Request(&retry))?;
    assert!(projected.projected_bytes() > 0);
    assert_eq!(policy.snapshot(), snapshot);
    let replacement = lease(&submit(&mut policy, retry)?)?;
    assert!(policy.retains_outage(old.resource));
    assert!(policy.retains_outage(replacement.resource));
    assert!(policy.retains_outage_resource(replacement.resource.id));
    assert_eq!(policy.retained_lease_associations(), 1);
    ready(&mut policy, replacement, 8188, true)?;
    policy.adopt_replacement(binding(1)?, old, replacement)?;
    assert!(!policy.retains_outage(old.resource));
    assert!(!policy.retains_outage(replacement.resource));
    assert!(!policy.retains_outage_resource(old.resource.id));
    assert_eq!(policy.snapshot().leases, 1);
    assert!(original.source().is_some());
    Ok(())
}

#[test]
fn second_death_preserves_exact_new_outage_and_does_not_allow_stale_adoption() -> TestResult {
    let (mut policy, old) = failed_granted()?;
    let original = settled_cause(&mut policy, old.resource)?;
    let replacement =
        lease(&submit(&mut policy, request(1, 2, ResourceCommand::RetryDead(old))?)?)?;
    ready(&mut policy, replacement, 8188, true)?;
    let observation = death(replacement, 23)?;
    let before = policy.snapshot();
    policy.storage_preflight(ResourceStorageInput::Observation(&observation))?;
    assert_eq!(policy.snapshot(), before);
    let transition = observe(&mut policy, observation)?;
    assert_eq!(
        transition.invalidations,
        vec![GrantedResourceInvalidation { lease: replacement, status: 23 }]
    );
    assert!(!policy.retains_outage(old.resource));
    assert!(policy.retains_outage(replacement.resource));
    assert!(policy.retains_outage_resource(old.resource.id));
    assert_eq!(
        policy.invalidated_lease(binding(1)?, replacement)?,
        InvalidatedLeaseState::Retiring
    );
    assert_eq!(policy.invalidated_lease(binding(1)?, old), Err(LifecycleRejection::StaleIdentity));
    assert_eq!(
        policy.adopt_replacement(binding(1)?, old, replacement),
        Err(LifecycleRejection::StaleIdentity)
    );
    assert!(original.source().is_some());
    Ok(())
}

#[test]
fn full_cohort_preflight_covers_both_256_entry_maps_without_aliasing_their_charge() -> TestResult {
    let mut policy = ResourceCoordinator::new(owner()?);
    let mut first = None;
    for instance in 1..=MAX_RUNS as u64 {
        register(&mut policy, instance, &format!("funded-cohort-{instance}"))?;
        for id in 1..=4 {
            let current = lease(&submit(
                &mut policy,
                request(instance, id, ResourceCommand::Acquire(target(8188, true)?))?,
            )?)?;
            first.get_or_insert(current);
        }
    }
    let old = first.ok_or("Missing cohort")?;
    ready(&mut policy, old, 8188, true)?;
    let admitted = policy.storage_preflight(ResourceStorageInput::Current)?;
    let observation = death(old, 7)?;
    let event = policy.storage_preflight(ResourceStorageInput::Observation(&observation))?;
    assert!(event.allocation_bytes() <= admitted.allocation_bytes());
    observe(&mut policy, observation)?;
    let dead = policy.storage_preflight(ResourceStorageInput::Current)?;
    assert!(dead.allocation_bytes() <= admitted.allocation_bytes());
    let original = settled_cause(&mut policy, old.resource)?;
    let before = policy.storage_preflight(ResourceStorageInput::Current)?;
    let retry = request(1, 5, ResourceCommand::RetryDead(old))?;
    let projected = policy.storage_preflight(ResourceStorageInput::Request(&retry))?;
    submit(&mut policy, retry)?;
    let after = policy.storage_preflight(ResourceStorageInput::Current)?;
    assert_eq!(policy.snapshot().leases, MAX_LEASES);
    assert_eq!(policy.retained_lease_associations(), MAX_LEASES);
    assert!(after.retained_bytes() > before.retained_bytes());
    assert!(projected.projected_bytes() >= after.retained_bytes() - before.retained_bytes());
    assert!(projected.transient_bytes() >= after.transient_bytes());
    assert!(original.source().is_some());
    Ok(())
}

#[test]
fn authoritative_retry_replay_survives_state_advance_without_fresh_admission() -> TestResult {
    let (mut policy, old) = failed_granted()?;
    let original = settled_cause(&mut policy, old.resource)?;
    let retry = request(1, 2, ResourceCommand::RetryDead(old))?;
    assert!(!policy.request_replay(binding(1)?, &retry)?);
    let replacement = lease(&submit(&mut policy, retry.clone())?)?;
    assert_eq!(policy.invalidated_lease(binding(1)?, old)?, InvalidatedLeaseState::Retrying);
    assert!(policy.request_replay(binding(1)?, &retry)?);
    assert_eq!(
        policy.storage_preflight(ResourceStorageInput::Request(&retry))?.projected_bytes(),
        0
    );
    let pending = submit(&mut policy, retry.clone())?;
    assert_eq!(pending.replies[0].outcome, RequestOutcome::Pending(replacement));
    assert!(pending.effects.is_empty());
    ready(&mut policy, replacement, 8188, true)?;
    assert!(policy.request_replay(binding(1)?, &retry)?);
    let granted = submit(&mut policy, retry)?;
    assert_eq!(granted.replies[0].outcome, RequestOutcome::Granted(replacement));
    assert!(granted.effects.is_empty() && granted.replacements.is_empty());
    let changed = request(1, 2, ResourceCommand::Reload(replacement))?;
    assert_eq!(
        policy.request_replay(binding(1)?, &changed),
        Err(LifecycleRejection::ChangedDuplicate)
    );
    let wrong = ClientBinding { connection: connection(99)?, ..binding(1)? };
    assert_eq!(policy.request_replay(wrong, &changed), Err(LifecycleRejection::StaleIdentity));
    assert!(original.source().is_some());
    Ok(())
}

// An empty coordinator has no possible 256-client native completion. Its
// requested-layout floor must leave genuine projection receiving possible.
#[test]
fn empty_and_small_admission_do_not_prepay_phantom_maximum_cohorts() -> TestResult {
    let mut policy = ResourceCoordinator::new(owner()?);
    let empty = policy.storage_preflight(ResourceStorageInput::Current)?;
    assert_eq!(empty.returned_transition_bytes(), 0);
    assert!(empty.allocation_bytes() < 256 * 1024);
    register(&mut policy, 1, "small-funded-run")?;
    let input = request(1, 1, ResourceCommand::Acquire(target(8188, true)?))?;
    let before = policy.snapshot();
    let required = policy.storage_preflight(ResourceStorageInput::Request(&input))?;
    let host_required = required
        .allocation_bytes()
        .checked_add(required.returned_transition_bytes().checked_mul(2).ok_or("copy overflow")?)
        .ok_or("requirement overflow")?;
    // This is App plus two result copies, not a claim that all Host/Parent roles fit.
    assert!(host_required < 1024 * 1024);
    assert_eq!(policy.snapshot(), before);
    assert!(required.returned_transition_bytes() < required.transient_bytes());
    let old_host_extra = required
        .projected_bytes()
        .checked_add(required.transient_bytes())
        .and_then(|bytes| bytes.checked_mul(2))
        .ok_or("old estimate overflow")?;
    assert!(2 * required.returned_transition_bytes() < old_host_extra);
    let returned = submit(&mut policy, input)?;
    assert!(transition_bytes(&returned)? <= required.returned_transition_bytes());
    Ok(())
}

#[test]
fn mandatory_ready_then_death_need_no_new_destination_credit() -> TestResult {
    let mut policy = ResourceCoordinator::new(owner()?);
    register(&mut policy, 1, "pending-observation")?;
    let request = request(1, 1, ResourceCommand::Acquire(target(8188, true)?))?;
    let admitted = policy.storage_preflight(ResourceStorageInput::Request(&request))?;
    let current = lease(&submit(&mut policy, request)?)?;
    let before_ready = policy.storage_preflight(ResourceStorageInput::Current)?;
    assert!(before_ready.allocation_bytes() <= admitted.allocation_bytes());
    ready(&mut policy, current, 8188, true)?;
    let after_ready = policy.storage_preflight(ResourceStorageInput::Current)?;
    assert!(after_ready.allocation_bytes() <= before_ready.allocation_bytes());
    let event = death(current, u32::MAX)?;
    let observed = policy.storage_preflight(ResourceStorageInput::Observation(&event))?;
    assert!(observed.allocation_bytes() <= after_ready.allocation_bytes());
    let result = observe(&mut policy, event)?;
    assert_eq!(result.invalidations.len(), 1);
    assert!(result.replies.is_empty());
    let after_death = policy.storage_preflight(ResourceStorageInput::Current)?;
    assert!(after_death.allocation_bytes() <= after_ready.allocation_bytes());
    assert!(policy.failure(current.resource).is_some_and(|cause| cause.source().is_some()));
    Ok(())
}

#[test]
fn disconnect_cancellations_and_cleanup_are_prepaid_for_actual_resources() -> TestResult {
    let mut policy = ResourceCoordinator::new(owner()?);
    register(&mut policy, 1, "disconnect-all")?;
    for id in 1..=MAX_RESOURCES as u64 {
        let port = 8200 + u16::try_from(id)?;
        submit(&mut policy, request(1, id, ResourceCommand::Acquire(target(port, true)?))?)?;
    }
    let admitted = policy.storage_preflight(ResourceStorageInput::Current)?;
    let request = ResourceStorageInput::Disconnect(binding(1)?);
    let mandatory = policy.storage_preflight(request)?;
    assert_eq!(mandatory, admitted);
    let transition = policy.disconnect_run(run(1)?, connection(1)?)?;
    assert_eq!(transition.replies.len(), MAX_RESOURCES);
    assert_eq!(transition.effects.len(), MAX_RESOURCES);
    assert!(transition.replies.iter().all(|reply| reply.outcome == RequestOutcome::Cancelled));
    let settled_policy = policy.storage_preflight(ResourceStorageInput::Current)?;
    assert!(settled_policy.allocation_bytes() <= admitted.allocation_bytes());
    assert_eq!(policy.snapshot().outstanding_requests, 0);
    assert_eq!(policy.snapshot().leases, 0);
    Ok(())
}

#[test]
fn returned_copy_bound_includes_actual_prepare_target_without_copying_input() -> TestResult {
    let mut policy = ResourceCoordinator::new(owner()?);
    register(&mut policy, 1, "returned-target")?;
    let input = request(1, 1, ResourceCommand::Acquire(managed_argument_capacity(256 * 1024)?))?;
    let ResourceCommand::Acquire(BackendTarget::Managed(target)) = &input.command else {
        return Err("Actual managed input missing".into());
    };
    let actual = u64::try_from(target.allocated_bytes().ok_or("actual capacity overflow")?)?;
    let snapshot = policy.snapshot();
    let required = policy.storage_preflight(ResourceStorageInput::Request(&input))?;
    assert!(required.returned_transition_bytes() >= actual);
    assert!(required.projected_bytes() >= actual * 3);
    let complete = required
        .allocation_bytes()
        .checked_add(required.returned_transition_bytes().checked_mul(2).ok_or("copy overflow")?)
        .ok_or("total overflow")?;
    assert_eq!(
        required.check_allocation_partition(required.allocation_bytes() - 1),
        Err(LifecycleRejection::Overloaded)
    );
    assert!(complete > required.allocation_bytes());
    assert_eq!(policy.snapshot(), snapshot);
    assert_eq!(target.allocated_bytes(), Some(usize::try_from(actual)?));
    Ok(())
}

#[test]
fn full_256_pending_outcomes_are_reserved_before_ready_is_observed() -> TestResult {
    let mut policy = ResourceCoordinator::new(owner()?);
    let mut first = None;
    for instance in 1..=MAX_RUNS as u64 {
        register(&mut policy, instance, &format!("pending-full-{instance}"))?;
        for id in 1..=4 {
            let acquired = lease(&submit(
                &mut policy,
                request(instance, id, ResourceCommand::Acquire(target(8188, true)?))?,
            )?)?;
            first.get_or_insert(acquired);
        }
    }
    assert_eq!(policy.snapshot().outstanding_requests, MAX_REQUESTS);
    assert_eq!(policy.snapshot().leases, MAX_LEASES);
    let admitted = policy.storage_preflight(ResourceStorageInput::Current)?;
    let current = first.ok_or("Full pending cohort missing")?;
    ready(&mut policy, current, 8188, true)?;
    let delivered = policy.storage_preflight(ResourceStorageInput::Current)?;
    assert_eq!(policy.snapshot().outstanding_requests, 0);
    assert_eq!(policy.snapshot().leases, MAX_LEASES);
    assert!(delivered.allocation_bytes() <= admitted.allocation_bytes());
    assert!(delivered.returned_transition_bytes() <= admitted.returned_transition_bytes());
    Ok(())
}

fn transition_bytes(value: &ResourceTransition) -> Result<u64, Box<dyn Error>> {
    let mut bytes = 0_u64;
    for (capacity, width) in [
        (value.replies.capacity(), size_of::<ResourceReply>()),
        (value.effects.capacity(), size_of::<ResourceEffect>()),
        (value.invalidations.capacity(), size_of::<GrantedResourceInvalidation>()),
        (value.replacements.capacity(), size_of::<GrantedResourceReplacement>()),
    ] {
        let allocation = u64::try_from(capacity)?
            .checked_mul(u64::try_from(width)?)
            .ok_or("Actual transition arithmetic overflow")?;
        bytes = bytes.checked_add(allocation).ok_or("Actual transition sum overflow")?;
    }
    for effect in &value.effects {
        if let ResourceEffect::Prepare { target: BackendTarget::Managed(target), .. } = effect {
            bytes = bytes
                .checked_add(u64::try_from(
                    target.allocated_bytes().ok_or("Actual target overflow")?,
                )?)
                .ok_or("Target sum overflow")?;
        }
    }
    Ok(bytes)
}
