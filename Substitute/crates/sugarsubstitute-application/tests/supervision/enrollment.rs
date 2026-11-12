// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove inert enrollment cannot substitute for authentic connection registration.
use super::support::*;
fn enrollment(instance: u64, profile: &str) -> Result<RunEnrollment, Box<dyn Error>> {
    Ok(RunEnrollment {
        run: run(instance)?,
        launch: LaunchId::new(instance)?,
        profile: ProfileId::new(profile.into())?,
    })
}
fn registration(enrollment: &RunEnrollment, connection: ConnectionId) -> ClientRegistration {
    ClientRegistration {
        run: enrollment.run,
        launch: enrollment.launch,
        profile: enrollment.profile.clone(),
        connection,
    }
}
#[test]
fn admitted_run_denies_acquire_reload_release_disconnect_and_unissued_readiness() -> TestResult {
    let mut policy = ResourceCoordinator::new(owner()?);
    let input = enrollment(1, "enrolled")?;
    policy.enroll_run(input.clone())?;
    policy.enroll_run(input)?;
    let before = policy.snapshot();
    let resource = ResourceKey {
        owner: owner()?,
        id: ResourceId::new(1)?,
        generation: ResourceGeneration::new(1)?,
    };
    let lease =
        LeaseKey { run: run(1)?, resource, connection: connection(1)?, id: LeaseId::new(1)? };
    for command in [
        ResourceCommand::Acquire(target(8188, false)?),
        ResourceCommand::Reload(lease),
        ResourceCommand::Release(lease),
    ] {
        assert_eq!(
            submit(&mut policy, request(1, 1, command)?),
            Err(LifecycleRejection::StaleIdentity)
        );
        assert_eq!(policy.snapshot(), before);
    }
    assert_eq!(
        policy.disconnect_run(run(1)?, connection(1)?),
        Err(LifecycleRejection::StaleIdentity)
    );
    assert_eq!(
        observe(
            &mut policy,
            ResourceObservation::Ready {
                resource,
                origin: LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?
            }
        ),
        Err(LifecycleRejection::StaleIdentity)
    );
    assert_eq!(policy.snapshot(), before);
    assert_eq!(
        (
            before.runs,
            before.retained_profiles,
            before.resources,
            before.leases,
            before.outstanding_requests
        ),
        (1, 1, 0, 0, 0)
    );
    Ok(())
}
#[test]
fn registration_requires_exact_existing_enrollment_and_preserves_it_on_rejection() -> TestResult {
    let mut policy = ResourceCoordinator::new(owner()?);
    let input = enrollment(1, "enrolled")?;
    let exact = registration(&input, connection(1)?);
    assert_eq!(policy.register_run(exact.clone()), Err(LifecycleRejection::StaleIdentity));
    assert_eq!(policy.snapshot().runs, 0);
    policy.enroll_run(input.clone())?;
    let before = policy.snapshot();
    let mut changed = exact.clone();
    changed.launch = LaunchId::new(2)?;
    assert_eq!(policy.register_run(changed), Err(LifecycleRejection::StaleIdentity));
    let mut changed = exact.clone();
    changed.profile = ProfileId::new("other".into())?;
    assert_eq!(policy.register_run(changed), Err(LifecycleRejection::StaleIdentity));
    let mut changed = exact.clone();
    changed.run.generation = changed.run.generation.next()?;
    assert_eq!(policy.register_run(changed), Err(LifecycleRejection::StaleIdentity));
    let mut changed = exact.clone();
    changed.run.owner.epoch = CoordinatorEpoch::new([9; 16])?;
    assert_eq!(policy.register_run(changed), Err(LifecycleRejection::ForeignOwner));
    let mut changed = input;
    changed.launch = LaunchId::new(2)?;
    assert_eq!(policy.enroll_run(changed), Err(LifecycleRejection::StaleIdentity));
    assert_eq!(policy.snapshot(), before);
    policy.register_run(exact)?;
    assert_eq!(
        submit(&mut policy, request(1, 1, ResourceCommand::Acquire(target(8188, false)?))?)?
            .effects
            .len(),
        1
    );
    Ok(())
}
#[test]
fn exact_connected_repeat_keeps_leases_request_history_and_current_connection() -> TestResult {
    let mut policy = ResourceCoordinator::new(owner()?);
    let input = enrollment(1, "enrolled")?;
    let exact = registration(&input, connection(1)?);
    policy.enroll_run(input.clone())?;
    policy.register_run(exact.clone())?;
    let request = request(1, 1, ResourceCommand::Acquire(target(8188, false)?))?;
    let first = submit(&mut policy, request.clone())?;
    let held = lease(&first)?;
    ready(&mut policy, held, 8188, false)?;
    let before = policy.snapshot();
    policy.register_run(exact.clone())?;
    policy.enroll_run(input)?;
    assert_eq!(policy.snapshot(), before);
    let repeated = submit(&mut policy, request)?;
    assert!(repeated.effects.is_empty());
    assert_eq!(repeated.replies[0].outcome, RequestOutcome::Granted(held));
    let mut changed = exact;
    changed.connection = connection(2)?;
    assert_eq!(policy.register_run(changed), Err(LifecycleRejection::StaleIdentity));
    assert_eq!(policy.snapshot(), before);
    Ok(())
}
#[test]
fn disconnected_enrollment_repeat_cannot_reconnect_or_reacquire_before_actual_settlement()
-> TestResult {
    let mut policy = ResourceCoordinator::new(owner()?);
    let input = enrollment(1, "enrolled")?;
    let exact = registration(&input, connection(1)?);
    policy.enroll_run(input.clone())?;
    policy.register_run(exact.clone())?;
    let held = lease(&submit(
        &mut policy,
        request(1, 1, ResourceCommand::Acquire(target(8188, false)?))?,
    )?)?;
    let disconnected = policy.disconnect_run(run(1)?, connection(1)?)?;
    assert_eq!(
        disconnected.effects,
        vec![ResourceEffect::DisconnectAttached { resource: held.resource }]
    );
    let before = policy.snapshot();
    policy.enroll_run(input.clone())?;
    for id in [1, 2] {
        assert_eq!(
            policy.register_run(registration(&input, connection(id)?)),
            Err(LifecycleRejection::StaleIdentity)
        );
    }
    assert_eq!(
        submit(&mut policy, request(1, 2, ResourceCommand::Acquire(target(8188, false)?))?),
        Err(LifecycleRejection::StaleIdentity)
    );
    assert_eq!(policy.settle_run(run(1)?), Err(LifecycleRejection::WrongPhase));
    assert_eq!(policy.snapshot(), before);
    observe(&mut policy, ResourceObservation::Settled(held.resource))?;
    policy.settle_run(run(1)?)?;
    assert_eq!(policy.register_run(exact), Err(LifecycleRejection::StaleIdentity));
    assert_eq!(policy.enroll_run(input), Err(LifecycleRejection::StaleIdentity));
    Ok(())
}
#[test]
fn enrollment_retains_profile_and_generation_until_adapter_settles_unlaunched_capsule() -> TestResult
{
    let mut policy = ResourceCoordinator::new(owner()?);
    let input = enrollment(1, "exclusive")?;
    policy.enroll_run(input.clone())?;
    assert_eq!(
        policy.enroll_run(enrollment(2, "exclusive")?),
        Err(LifecycleRejection::ProfileContention)
    );
    let mut next = input.clone();
    next.run.generation = next.run.generation.next()?;
    assert_eq!(policy.enroll_run(next.clone()), Err(LifecycleRejection::WrongPhase));
    // This owner test invokes the trusted settlement boundary for an unlaunched
    // capsule. Enrollment, absent connection and elapsed time never call it.
    policy.settle_run(input.run)?;
    assert_eq!(
        policy.register_run(registration(&input, connection(1)?)),
        Err(LifecycleRejection::StaleIdentity)
    );
    policy.enroll_run(next.clone())?;
    let mut new_registration = registration(&next, connection(2)?);
    new_registration.launch = LaunchId::new(2)?;
    assert_eq!(policy.register_run(new_registration), Err(LifecycleRejection::StaleIdentity));
    policy.register_run(registration(&next, connection(2)?))?;
    policy.enroll_run(enrollment(2, "other")?)?;
    assert_eq!(policy.snapshot().retained_profiles, 2);
    Ok(())
}
#[test]
fn all64_admitted_runs_enforce_original_capacity_and_evicted_identity_highwater() -> TestResult {
    let mut policy = ResourceCoordinator::new(owner()?);
    for instance in 1..=64 {
        policy.enroll_run(enrollment(instance, &format!("profile{instance}"))?)?;
    }
    let before = policy.snapshot();
    assert_eq!(policy.enroll_run(enrollment(65, "extra")?), Err(LifecycleRejection::Overloaded));
    assert_eq!(policy.snapshot(), before);
    assert_eq!((before.runs, before.retained_profiles, before.resources), (MAX_RUNS, MAX_RUNS, 0));
    policy.settle_run(run(1)?)?;
    policy.enroll_run(enrollment(65, "extra")?)?;
    let mut old = enrollment(1, "old")?;
    old.run.generation = old.run.generation.next()?;
    assert_eq!(policy.enroll_run(old), Err(LifecycleRejection::StaleIdentity));
    assert_eq!(policy.snapshot().runs, MAX_RUNS);
    Ok(())
}
