// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drive actual Host campaign/policy owners with explicit external registry observations.
use super::*;
#[test]
fn exact_contained_start_alone_admits_live_wait_and_matching_exit_fails_pending_acquire()
-> Result<()> {
    let clock = std::sync::Arc::new(tests::TestClock::default());
    let executor = sugarsubstitute_execution::ExecutionOwner::new(
        clock.clone(),
        std::sync::Arc::new(tests::Wake),
    )?;
    let client = executor.client();
    let scope = client.create_scope(
        ScopeSpec { id: sugarsubstitute_contracts::execution::ScopeId::new(1)?, deadline: None },
        None,
    )?;
    let mut held = tests::unadmitted_policy()?;
    let backend = held.backend.as_mut();
    let run = run(tests::key()?, 1)?;
    let registration = registration(run)?;
    backend.policy.enroll_run(RunEnrollment {
        run,
        launch: registration.launch,
        profile: registration.profile.clone(),
    })?;
    backend.register_run(registration)?;
    let target =
        BackendTarget::Managed(sugarsubstitute_contracts::configuration::LaunchConfiguration::new(
            r"E:\unopened-live-test".into(),
            r"E:\unopened-live-test\python.exe".into(),
            vec![],
            8188,
        )?);
    let transition =
        submit(&mut backend.policy, request(run, 1, ResourceCommand::Acquire(target.clone()))?)?;
    let lease = lease(&transition)?;
    let key = lease.resource;
    backend.records[0] = Some(creator(
        key,
        target,
        ConnectionBudget::new(ConnectionBudgetKind::Managed, Moment::from_nanos(0))?,
        scope,
    )?);
    backend.observe(ResourceObservation::ClaimAcquired(key))?;
    let process = ProcessKey { owner: key.owner, id: ProcessId::new(7)? };
    let other = ProcessKey { id: ProcessId::new(8)?, ..process };
    assert!(
        backend.managed_started_observation(key, process, other, Moment::from_nanos(1)).is_err()
    );
    assert!(!backend.record(key)?.started);
    assert_eq!(
        backend.record(key)?.campaign.campaign_until(),
        Some(Moment::from_nanos(180_000_000_000))
    );
    let started =
        backend.managed_started_observation(key, process, process, Moment::from_nanos(1))?;
    assert!(started.replies.is_empty());
    assert!(backend.record(key)?.started);
    clock.set(Moment::from_nanos(360_000_000_000));
    assert_eq!(backend.record(key)?.campaign.campaign_until(), None);
    assert_eq!(
        backend.record(key)?.campaign.probe_until(client.now()?)?,
        Moment::from_nanos(362_000_000_000)
    );
    assert_eq!(backend.record(key)?.scope.spec().deadline, None);
    assert!(!backend.record(key)?.scope.snapshot().cancellation_requested);
    assert!(backend.managed_exit_observation(key, process, other, 3).is_err());
    let exited = backend.managed_exit_observation(key, process, process, 3)?;
    assert_eq!(exited.replies.len(), 1);
    assert_eq!(exited.replies[0].outcome, RequestOutcome::Failed);
    assert_eq!(backend.record(key)?.managed_exit, Some((process, 3)));
    assert!(!backend.record(key)?.settled);
    Ok(())
}
