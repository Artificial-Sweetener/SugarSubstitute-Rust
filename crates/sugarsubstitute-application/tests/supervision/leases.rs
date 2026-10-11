// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
use super::support::*;
#[test]
fn shared_managed_leases_reload_and_last_release_stop_once() -> TestResult {
    let mut coordinator = ResourceCoordinator::new(owner()?);
    register(&mut coordinator, 1, "first")?;
    register(&mut coordinator, 2, "second")?;
    let first_request = request(1, 1, ResourceCommand::Acquire(target(8188, true)?))?;
    let first = submit(&mut coordinator, first_request.clone())?;
    let a = lease(&first)?;
    assert_eq!(first.effects.len(), 1);
    assert!(submit(&mut coordinator, first_request)?.effects.is_empty());
    let b = lease(&submit(
        &mut coordinator,
        request(2, 1, ResourceCommand::Acquire(target(8188, true)?))?,
    )?)?;
    assert_eq!(a.resource, b.resource);
    ready(&mut coordinator, a, 8188, true)?;
    let reload = submit(&mut coordinator, request(1, 2, ResourceCommand::Reload(a))?)?;
    assert_eq!(reload.replies[0].outcome, RequestOutcome::Reloaded(a));
    assert!(reload.effects.is_empty());
    assert!(
        submit(&mut coordinator, request(1, 3, ResourceCommand::Release(a))?)?.effects.is_empty()
    );
    let release = request(2, 2, ResourceCommand::Release(b))?;
    assert_eq!(
        submit(&mut coordinator, release.clone())?.effects,
        vec![ResourceEffect::StopOwned { resource: a.resource }]
    );
    assert!(submit(&mut coordinator, release)?.effects.is_empty());
    assert_eq!(coordinator.resource_phase(a.resource), Some(ResourcePhase::Stopping));
    assert_eq!(
        submit(&mut coordinator, request(1, 4, ResourceCommand::Acquire(target(8188, true)?))?),
        Err(LifecycleRejection::WrongPhase)
    );
    observe(&mut coordinator, ResourceObservation::Settled(a.resource))?;
    let next = lease(&submit(
        &mut coordinator,
        request(1, 4, ResourceCommand::Acquire(target(8188, true)?))?,
    )?)?;
    assert_eq!(next.resource.id, a.resource.id);
    assert!(next.resource.generation > a.resource.generation);
    assert_eq!(
        observe(&mut coordinator, ResourceObservation::Settled(a.resource)),
        Err(LifecycleRejection::StaleIdentity)
    );
    Ok(())
}

#[test]
fn attachment_disconnect_never_stops_external_process_and_profile_waits_for_settlement()
-> TestResult {
    let mut coordinator = ResourceCoordinator::new(owner()?);
    register(&mut coordinator, 1, "exclusive")?;
    assert!(register(&mut coordinator, 2, "exclusive").is_err());
    assert_eq!(coordinator.snapshot().resources, 0);
    let a = lease(&submit(
        &mut coordinator,
        request(1, 1, ResourceCommand::Acquire(target(8188, false)?))?,
    )?)?;
    ready(&mut coordinator, a, 8188, false)?;
    assert_eq!(
        coordinator.disconnect_run(run(1)?, connection(1)?)?.effects,
        vec![ResourceEffect::DisconnectAttached { resource: a.resource }]
    );
    assert!(register(&mut coordinator, 2, "exclusive").is_err());
    assert_eq!(coordinator.settle_run(run(1)?), Err(LifecycleRejection::WrongPhase));
    observe(&mut coordinator, ResourceObservation::Settled(a.resource))?;
    assert!(register(&mut coordinator, 2, "exclusive").is_err());
    coordinator.settle_run(run(1)?)?;
    register(&mut coordinator, 2, "exclusive")?;
    Ok(())
}

#[test]
fn cancelled_pending_native_creation_remains_owned_through_late_claim_and_launch() -> TestResult {
    let mut coordinator = ResourceCoordinator::new(owner()?);
    register(&mut coordinator, 1, "first")?;
    let a = lease(&submit(
        &mut coordinator,
        request(1, 1, ResourceCommand::Acquire(target(8188, true)?))?,
    )?)?;
    let transition = coordinator.disconnect_run(run(1)?, connection(1)?)?;
    assert_eq!(transition.replies[0].outcome, RequestOutcome::Cancelled);
    assert_eq!(transition.effects, vec![ResourceEffect::StopOwned { resource: a.resource }]);
    observe(&mut coordinator, ResourceObservation::ClaimAcquired(a.resource))?;
    observe(
        &mut coordinator,
        ResourceObservation::OwnedStarted {
            resource: a.resource,
            process: ProcessKey { owner: owner()?, id: ProcessId::new(1)? },
        },
    )?;
    assert_eq!(coordinator.snapshot().resources, 1);
    assert_eq!(coordinator.settle_run(run(1)?), Err(LifecycleRejection::WrongPhase));
    assert_eq!(coordinator.resource_phase(a.resource), Some(ResourcePhase::Stopping));
    observe(&mut coordinator, ResourceObservation::Settled(a.resource))?;
    coordinator.settle_run(run(1)?)?;
    Ok(())
}

#[test]
fn failures_return_rejected_causes_and_require_settlement_then_explicit_cause_transfer()
-> TestResult {
    let mut coordinator = ResourceCoordinator::new(owner()?);
    register(&mut coordinator, 1, "first")?;
    let a = lease(&submit(
        &mut coordinator,
        request(1, 1, ResourceCommand::Acquire(target(8188, true)?))?,
    )?)?;
    observe(
        &mut coordinator,
        ResourceObservation::Failed {
            resource: a.resource,
            failure: LifecycleFailure::new(
                LifecycleFailureCode::Preparation,
                Box::new(std::io::Error::other("secret cause")),
            ),
        },
    )?;
    assert_eq!(coordinator.snapshot().outstanding_requests, 0);
    assert_eq!(
        coordinator.take_failure(a.resource).map(|_| ()),
        Err(LifecycleRejection::WrongPhase)
    );
    let duplicate = coordinator.observe(ResourceObservation::Failed {
        resource: a.resource,
        failure: LifecycleFailure::new(
            LifecycleFailureCode::Cleanup,
            Box::new(std::io::Error::other("second cause")),
        ),
    });
    let rejected = duplicate.err().ok_or("Repeated failure accepted")?;
    assert_eq!(rejected.reason, LifecycleRejection::WrongPhase);
    if let ResourceObservation::Failed { failure, .. } = rejected.observation {
        assert_eq!(failure.source().ok_or("Missing cause")?.to_string(), "second cause");
    } else {
        return Err("Cause not returned".into());
    }
    observe(&mut coordinator, ResourceObservation::Settled(a.resource))?;
    assert_eq!(
        submit(&mut coordinator, request(1, 2, ResourceCommand::Acquire(target(8188, true)?))?),
        Err(LifecycleRejection::WrongPhase)
    );
    let failure = coordinator.take_failure(a.resource)?.ok_or("First cause missing")?;
    assert!(!format!("{failure:?} {failure}").contains("secret"));
    assert_eq!(failure.source().ok_or("Missing source")?.to_string(), "secret cause");
    submit(&mut coordinator, request(1, 2, ResourceCommand::Acquire(target(8188, true)?))?)?;
    Ok(())
}
