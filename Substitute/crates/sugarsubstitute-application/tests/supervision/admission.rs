// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
use super::support::*;
#[test]
fn foreign_connections_runs_epochs_and_stolen_leases_reject_without_mutation() -> TestResult {
    let mut coordinator = ResourceCoordinator::new(owner()?);
    register(&mut coordinator, 1, "first")?;
    register(&mut coordinator, 2, "second")?;
    let a = lease(&submit(
        &mut coordinator,
        request(1, 1, ResourceCommand::Acquire(target(8188, false)?))?,
    )?)?;
    let before = coordinator.snapshot();
    assert_eq!(
        coordinator.request(
            ClientBinding { run: run(1)?, connection: connection(1)? },
            request(2, 1, ResourceCommand::Acquire(target(8188, false)?))?
        ),
        Err(LifecycleRejection::StaleIdentity)
    );
    assert_eq!(
        submit(&mut coordinator, request(2, 1, ResourceCommand::Release(a))?),
        Err(LifecycleRejection::StaleIdentity)
    );
    let mut wrong = request(1, 2, ResourceCommand::Release(a))?;
    wrong.connection = connection(99)?;
    assert_eq!(submit(&mut coordinator, wrong), Err(LifecycleRejection::StaleIdentity));
    let mut wrong = request(1, 2, ResourceCommand::Release(a))?;
    wrong.run.owner.epoch = CoordinatorEpoch::new([9; 16])?;
    assert_eq!(submit(&mut coordinator, wrong), Err(LifecycleRejection::ForeignOwner));
    let mut wrong = request(1, 2, ResourceCommand::Release(a))?;
    wrong.run.generation = RunGeneration::new(2)?;
    assert_eq!(submit(&mut coordinator, wrong), Err(LifecycleRejection::StaleIdentity));
    assert_eq!(coordinator.snapshot(), before);
    let port = NonZeroU16::new(8189).ok_or("Zero")?;
    assert_eq!(
        observe(
            &mut coordinator,
            ResourceObservation::Ready {
                resource: a.resource,
                origin: LoopbackOrigin::parse("http://127.0.0.1:8189", port.get())?
            }
        ),
        Err(LifecycleRejection::EndpointMismatch)
    );
    assert_eq!(coordinator.snapshot(), before);
    Ok(())
}

#[test]
fn run_resource_and_per_run_pending_capacity_rejections_preserve_records() -> TestResult {
    let mut coordinator = ResourceCoordinator::new(owner()?);
    for instance in 1..=64 {
        register(&mut coordinator, instance, &format!("profile{instance}"))?;
    }
    assert!(register(&mut coordinator, 65, "extra").is_err());
    assert_eq!(coordinator.snapshot().runs, MAX_RUNS);
    for id in 1..=64 {
        submit(
            &mut coordinator,
            request(1, id, ResourceCommand::Acquire(target(8188 + u16::try_from(id)?, true)?))?,
        )?;
    }
    let before = coordinator.snapshot();
    assert_eq!(
        submit(&mut coordinator, request(1, 65, ResourceCommand::Acquire(target(8190, true)?))?),
        Err(LifecycleRejection::Overloaded)
    );
    assert_eq!(
        submit(&mut coordinator, request(2, 1, ResourceCommand::Acquire(target(9000, true)?))?),
        Err(LifecycleRejection::Overloaded)
    );
    assert_eq!(coordinator.snapshot(), before);
    assert_eq!(before.resources, MAX_RESOURCES);
    assert_eq!(before.outstanding_requests, MAX_RUN_REQUESTS);
    Ok(())
}

#[test]
fn request_history_eviction_and_changed_duplicates_never_repeat_side_effects() -> TestResult {
    let mut coordinator = ResourceCoordinator::new(owner()?);
    register(&mut coordinator, 1, "first")?;
    let first = request(1, 1, ResourceCommand::Acquire(target(8188, false)?))?;
    let a = lease(&submit(&mut coordinator, first.clone())?)?;
    assert_eq!(
        submit(&mut coordinator, request(1, 1, ResourceCommand::Acquire(target(8189, false)?))?),
        Err(LifecycleRejection::ChangedDuplicate)
    );
    ready(&mut coordinator, a, 8188, false)?;
    for id in 2..=70 {
        submit(&mut coordinator, request(1, id, ResourceCommand::Reload(a))?)?;
    }
    assert_eq!(submit(&mut coordinator, first), Err(LifecycleRejection::StaleRequest));
    let repeated = submit(&mut coordinator, request(1, 70, ResourceCommand::Reload(a))?)?;
    assert_eq!(repeated.replies[0].outcome, RequestOutcome::Reloaded(a));
    assert!(repeated.effects.is_empty());
    Ok(())
}

#[test]
fn pending_request_and_lease_limits_include_retained_work_and_allow_cleanup() -> TestResult {
    let mut coordinator = ResourceCoordinator::new(owner()?);
    for instance in 1..=4 {
        register(&mut coordinator, instance, &format!("profile{instance}"))?;
    }
    let mut first = None;
    for instance in 1..=4 {
        for id in 1..=64 {
            let a = lease(&submit(
                &mut coordinator,
                request(instance, id, ResourceCommand::Acquire(target(8188, true)?))?,
            )?)?;
            first = first.or(Some(a));
        }
    }
    assert_eq!(coordinator.snapshot().leases, MAX_LEASES);
    assert_eq!(coordinator.snapshot().outstanding_requests, MAX_REQUESTS);
    let before = coordinator.snapshot();
    assert_eq!(
        submit(&mut coordinator, request(1, 65, ResourceCommand::Acquire(target(8188, true)?))?),
        Err(LifecycleRejection::Overloaded)
    );
    assert_eq!(coordinator.snapshot(), before);
    submit(
        &mut coordinator,
        request(1, 65, ResourceCommand::Release(first.ok_or("Lease missing")?))?,
    )?;
    assert_eq!(coordinator.snapshot().outstanding_requests, MAX_REQUESTS - 1);
    submit(&mut coordinator, request(1, 66, ResourceCommand::Acquire(target(8188, true)?))?)?;
    Ok(())
}

#[test]
fn sequential_settled_resources_do_not_grow_run_tracking_and_uncertain_cleanup_blocks() -> TestResult
{
    let mut coordinator = ResourceCoordinator::new(owner()?);
    register(&mut coordinator, 1, "first")?;
    for index in 0..130u16 {
        let a = lease(&submit(
            &mut coordinator,
            request(
                1,
                u64::from(index) * 2 + 1,
                ResourceCommand::Acquire(target(8188 + index, false)?),
            )?,
        )?)?;
        submit(
            &mut coordinator,
            request(1, u64::from(index) * 2 + 2, ResourceCommand::Release(a))?,
        )?;
        observe(&mut coordinator, ResourceObservation::Settled(a.resource))?;
        assert_eq!(coordinator.snapshot().resources, 0);
        assert_eq!(coordinator.snapshot().tracked_resource_refs, 0);
    }
    let a = lease(&submit(
        &mut coordinator,
        request(1, 261, ResourceCommand::Acquire(target(9000, true)?))?,
    )?)?;
    coordinator.disconnect_run(run(1)?, connection(1)?)?;
    assert_eq!(coordinator.settle_run(run(1)?), Err(LifecycleRejection::WrongPhase));
    observe(&mut coordinator, ResourceObservation::Settled(a.resource))?;
    coordinator.settle_run(run(1)?)?;
    Ok(())
}
