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

#[test]
fn attempt_release_preserves_other_clients_and_requires_actual_last_lease_settlement() -> TestResult
{
    let mut coordinator = ResourceCoordinator::new(owner()?);
    register(&mut coordinator, 1, "first")?;
    register(&mut coordinator, 2, "second")?;
    let a = lease(&submit(
        &mut coordinator,
        request(1, 1, ResourceCommand::Acquire(target(8188, true)?))?,
    )?)?;
    let b = lease(&submit(
        &mut coordinator,
        request(2, 1, ResourceCommand::Acquire(target(8188, true)?))?,
    )?)?;
    ready(&mut coordinator, a, 8188, true)?;
    let mut transition = ResourceTransition::default();
    assert_eq!(
        coordinator.release_attempt(
            ClientBinding { run: run(2)?, connection: connection(2)? },
            a,
            &mut transition
        ),
        Err(LifecycleRejection::StaleIdentity)
    );
    assert_eq!(coordinator.snapshot().leases, 2);
    assert!(
        !coordinator
            .attempt_released(ClientBinding { run: run(1)?, connection: connection(1)? }, a)?
    );
    coordinator.release_attempt(
        ClientBinding { run: run(1)?, connection: connection(1)? },
        a,
        &mut transition,
    )?;
    assert!(transition.effects.is_empty());
    assert_eq!(coordinator.snapshot().leases, 1);
    assert!(
        coordinator
            .attempt_released(ClientBinding { run: run(1)?, connection: connection(1)? }, a)?
    );
    assert_eq!(coordinator.resource_phase(a.resource), Some(ResourcePhase::Ready));
    coordinator.release_attempt(
        ClientBinding { run: run(2)?, connection: connection(2)? },
        b,
        &mut transition,
    )?;
    assert_eq!(transition.effects, vec![ResourceEffect::StopOwned { resource: b.resource }]);
    assert_eq!(coordinator.resource_phase(b.resource), Some(ResourcePhase::Stopping));
    assert!(
        submit(&mut coordinator, request(1, 2, ResourceCommand::Acquire(target(8188, true)?))?)
            .is_err()
    );
    observe(&mut coordinator, ResourceObservation::Settled(b.resource))?;
    let next = lease(&submit(
        &mut coordinator,
        request(1, 3, ResourceCommand::Acquire(target(8188, true)?))?,
    )?)?;
    assert!(next.resource.generation.get() > b.resource.generation.get());
    Ok(())
}

#[test]
fn managed_provisional_clients_keep_other_wait_after_elapsed_preparation_and_fail_on_exact_exit()
-> TestResult {
    use sugarsubstitute_application::startup::{
        ConnectionBudget, ConnectionBudgetKind, ProbeBudgetError,
    };
    use sugarsubstitute_contracts::execution::Moment;

    let mut coordinator = ResourceCoordinator::new(owner()?);
    register(&mut coordinator, 1, "managed-wait-first")?;
    register(&mut coordinator, 2, "managed-wait-second")?;
    let first_request = request(1, 1, ResourceCommand::Acquire(target(8188, true)?))?;
    let second_request = request(2, 1, ResourceCommand::Acquire(target(8188, true)?))?;
    let first = lease(&submit(&mut coordinator, first_request.clone())?)?;
    let second = lease(&submit(&mut coordinator, second_request.clone())?)?;
    assert_eq!(first.resource, second.resource);
    assert_ne!(first.id, second.id);
    observe(&mut coordinator, ResourceObservation::ClaimAcquired(first.resource))?;
    let process = ProcessKey { owner: owner()?, id: ProcessId::new(1)? };
    observe(
        &mut coordinator,
        ResourceObservation::OwnedStarted { resource: first.resource, process },
    )?;
    let before = coordinator.snapshot();
    assert_eq!(before.leases, 2);
    assert_eq!(before.outstanding_requests, 2);
    // This public policy receives trusted process facts; it creates no native proof.
    let preparation = ConnectionBudget::new(ConnectionBudgetKind::Managed, Moment::from_nanos(0))?;
    let now = Moment::from_nanos(600_000_000_000);
    assert_eq!(preparation.probe_until(now), Err(ProbeBudgetError::Expired));
    assert_eq!(preparation.readiness_probe_until(now)?, Moment::from_nanos(602_000_000_000));
    assert_eq!(coordinator.snapshot(), before);
    let mut cancelled = ResourceTransition::default();
    coordinator.release_attempt(
        ClientBinding { run: first.run, connection: first.connection },
        first,
        &mut cancelled,
    )?;
    assert!(cancelled.effects.is_empty());
    assert_eq!(
        cancelled.replies,
        vec![ResourceReply {
            run: first.run,
            request: first_request.id,
            outcome: RequestOutcome::Cancelled
        }]
    );
    assert_eq!(coordinator.snapshot().leases, 1);
    assert_eq!(coordinator.snapshot().outstanding_requests, 1);
    assert_eq!(coordinator.resource_phase(second.resource), Some(ResourcePhase::Starting));
    assert_eq!(
        submit(&mut coordinator, second_request.clone())?.replies[0].outcome,
        RequestOutcome::Pending(second)
    );
    let wrong = coordinator
        .observe(ResourceObservation::ManagedExited {
            resource: second.resource,
            process: ProcessKey { id: ProcessId::new(2)?, ..process },
            status: 37,
            failure: LifecycleFailure::new(
                LifecycleFailureCode::UnexpectedExit,
                Box::new(std::io::Error::other("foreign process")),
            ),
        })
        .err()
        .ok_or("Foreign process exit accepted")?;
    assert_eq!(wrong.reason, LifecycleRejection::WrongPhase);
    assert_eq!(coordinator.snapshot().outstanding_requests, 1);
    let exited = observe(
        &mut coordinator,
        ResourceObservation::ManagedExited {
            resource: second.resource,
            process,
            status: 37,
            failure: LifecycleFailure::new(
                LifecycleFailureCode::UnexpectedExit,
                Box::new(std::io::Error::other("original managed exit")),
            ),
        },
    )?;
    assert_eq!(
        exited.replies,
        vec![ResourceReply {
            run: second.run,
            request: second_request.id,
            outcome: RequestOutcome::Failed
        }]
    );
    assert_eq!(exited.effects, vec![ResourceEffect::StopOwned { resource: second.resource }]);
    assert!(exited.invalidations.is_empty());
    assert_eq!(coordinator.snapshot().leases, 0);
    assert_eq!(coordinator.snapshot().outstanding_requests, 0);
    assert_eq!(
        submit(&mut coordinator, first_request)?.replies[0].outcome,
        RequestOutcome::Cancelled
    );
    assert_eq!(
        submit(&mut coordinator, second_request)?.replies[0].outcome,
        RequestOutcome::Failed
    );
    assert_eq!(
        coordinator.take_failure(second.resource).map(|_| ()),
        Err(LifecycleRejection::WrongPhase)
    );
    observe(&mut coordinator, ResourceObservation::Settled(second.resource))?;
    let failure =
        coordinator.take_failure(second.resource)?.ok_or("Original exit cause missing")?;
    assert_eq!(failure.source().ok_or("Missing exit source")?.to_string(), "original managed exit");
    Ok(())
}
