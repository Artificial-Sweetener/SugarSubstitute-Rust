// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve actual backend failure ownership without converting cleanup into IPC failure.
use super::*;
use std::sync::Arc;
#[test]
fn later_cleanup_failures_preserve_original_policy_cause_and_original_scope_budget() -> Result<()> {
    let executor = sugarsubstitute_execution::ExecutionOwner::new(
        Arc::new(tests::TestClock::default()),
        Arc::new(tests::Wake),
    )?;
    let client = executor.client();
    let scopes = ScopeSequence::new();
    let native =
        client.create_native_scope(ScopeSpec { id: scopes.allocate()?, deadline: None }, None)?;
    let cleanup =
        client.create_scope(ScopeSpec { id: scopes.allocate()?, deadline: None }, None)?;
    let scope =
        client.create_scope(ScopeSpec { id: scopes.allocate()?, deadline: None }, Some(&native))?;
    let actual_scope = scope.id();
    let jobs = JobSequence::new();
    let mut authority = CoordinatorAuthority::new(
        sugarsubstitute_contracts::coordinator_entry::EntrySetupBudget::new(Moment::from_nanos(0))?,
    )?;
    let mut held = tests::unadmitted_policy()?;
    let backend = held.backend.as_mut();
    let run = RunKey {
        owner: tests::key()?,
        instance: InstanceId::new(1)?,
        generation: RunGeneration::new(1)?,
    };
    let connection = ConnectionId::new(1)?;
    backend.register_run(ClientRegistration {
        run,
        connection,
        launch: sugarsubstitute_contracts::startup::LaunchId::new(1)?,
        profile: sugarsubstitute_contracts::configuration::ProfileId::new("Cleanup_1".into())?,
    })?;
    let target = BackendTarget::Attached(LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?);
    let transition = backend.policy.request(
        ClientBinding { run, connection },
        ResourceRequest {
            run,
            connection,
            id: RequestId::new(1)?,
            command: ResourceCommand::Acquire(target.clone()),
        },
    )?;
    let Some(ResourceEffect::Prepare { resource: key, .. }) = transition.effects.first() else {
        return Err("Actual policy did not retain its preparation generation".into());
    };
    let key = *key;
    backend.records[0] = Some(cutoff_tests::creator(
        key,
        target,
        ConnectionBudget::new(ConnectionBudgetKind::Attached, Moment::from_nanos(0))?,
        scope,
    )?);
    let mut resources = BackendResources {
        client: &client,
        native: &native,
        cleanup: &cleanup,
        jobs: &jobs,
        scopes: &scopes,
        authority: &mut authority,
        capture: None,
    };
    backend.begin_delivery()?;
    let first = Box::new(std::io::Error::from_raw_os_error(122));
    let first_pointer = std::ptr::from_ref(first.as_ref());
    let failed = backend.failed(key, first, &mut resources)?;
    assert_eq!(failed.replies[0].outcome, RequestOutcome::Failed);
    let original = backend.record(key)?.shutdown.ok_or("Actual cleanup budget missing")?;
    assert_eq!(backend.next_deadline(), Some(original.1.graceful_until));
    assert!(backend.record(key)?.scope.snapshot().cancellation_requested);
    let second = Box::new(std::io::Error::from_raw_os_error(5));
    let second_pointer = std::ptr::from_ref(second.as_ref());
    let later = backend.failed(key, second, &mut resources)?;
    assert!(later.replies.is_empty() && later.effects.is_empty());
    backend.failed(key, Box::new(std::io::Error::from_raw_os_error(6)), &mut resources)?;
    assert_eq!(
        backend
            .failure(key)
            .and_then(Error::source)
            .and_then(|e| e.downcast_ref::<std::io::Error>())
            .map(std::ptr::from_ref),
        Some(first_pointer)
    );
    assert_eq!(
        backend
            .cleanup_failure(key)
            .and_then(|e| e.downcast_ref::<std::io::Error>())
            .map(std::ptr::from_ref),
        Some(second_pointer)
    );
    assert_eq!(backend.record(key)?.shutdown, Some(original));
    assert_eq!(backend.campaign_scope(key), Some(actual_scope));
    assert!(!backend.attempt_settled(key));
    assert!(backend.take_attempt_failure(key).is_err());
    assert_eq!(native.snapshot().reserved_bytes, 0);
    assert_eq!(cleanup.snapshot().reserved_bytes, 0);
    assert_eq!(jobs.allocate()?.get(), 1);
    Ok(())
}

#[test]
fn late_original_grace_admission_progresses_actual_inert_cleanup_without_another_wake() -> Result<()>
{
    for elapsed in [13_000_000_000, 21_000_000_000] {
        let clock = Arc::new(tests::TestClock::default());
        let executor =
            sugarsubstitute_execution::ExecutionOwner::new(clock.clone(), Arc::new(tests::Wake))?;
        let client = executor.client();
        let scopes = ScopeSequence::new();
        let native = client
            .create_native_scope(ScopeSpec { id: scopes.allocate()?, deadline: None }, None)?;
        let cleanup =
            client.create_scope(ScopeSpec { id: scopes.allocate()?, deadline: None }, None)?;
        let scope = client
            .create_scope(ScopeSpec { id: scopes.allocate()?, deadline: None }, Some(&native))?;
        let jobs = JobSequence::new();
        let mut authority = CoordinatorAuthority::new(
            sugarsubstitute_contracts::coordinator_entry::EntrySetupBudget::new(
                Moment::from_nanos(0),
            )?,
        )?;
        let mut held = tests::unadmitted_policy()?;
        let backend = held.backend.as_mut();
        let run = RunKey {
            owner: tests::key()?,
            instance: InstanceId::new(1)?,
            generation: RunGeneration::new(1)?,
        };
        let connection = ConnectionId::new(1)?;
        backend.register_run(ClientRegistration {
            run,
            connection,
            launch: sugarsubstitute_contracts::startup::LaunchId::new(1)?,
            profile: sugarsubstitute_contracts::configuration::ProfileId::new("Late_1".into())?,
        })?;
        let target = BackendTarget::Attached(LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?);
        let produced = backend.policy.request(
            ClientBinding { run, connection },
            ResourceRequest {
                run,
                connection,
                id: RequestId::new(1)?,
                command: ResourceCommand::Acquire(target.clone()),
            },
        )?;
        let Some(ResourceEffect::Prepare { resource: key, .. }) = produced.effects.first() else {
            return Err("Actual preparation generation absent".into());
        };
        let key = *key;
        // No source/probe/native job was admitted. These genuine inert owners retain
        // no native output, so checked cleanup can settle their failed policy generation.
        backend.records[0] = Some(cutoff_tests::creator(
            key,
            target,
            ConnectionBudget::new(ConnectionBudgetKind::Attached, Moment::from_nanos(0))?,
            scope,
        )?);
        backend.begin_delivery()?;
        backend.observe(ResourceObservation::Failed {
            resource: key,
            failure: LifecycleFailure::new(
                LifecycleFailureCode::Preparation,
                Box::new(std::io::Error::from_raw_os_error(122)),
            ),
        })?;
        clock.set(Moment::from_nanos(elapsed));
        let mut resources = BackendResources {
            client: &client,
            native: &native,
            cleanup: &cleanup,
            jobs: &jobs,
            scopes: &scopes,
            authority: &mut authority,
            capture: None,
        };
        let original = ShutdownBudget::new(Moment::from_nanos(0))?;
        let transition = backend.begin_cleanup(key, Some(original), &mut resources)?;
        assert!(transition.effects.is_empty());
        assert!(backend.attempt_settled(key));
        assert_eq!(backend.record(key)?.shutdown.map(|(_, budget)| budget), Some(original));
        assert!(backend.next_deadline().is_none());
        assert_eq!(jobs.allocate()?.get(), 1);
        // The actual produced failure remains routed, rather than lost with immediate cleanup.
        let pending = backend.take_pending_transition().ok_or("Actual pending failure absent")?;
        assert_eq!(pending.replies[0].outcome, RequestOutcome::Failed);
        if elapsed >= original.settle_until.as_nanos() {
            assert_eq!(
                backend
                    .cleanup_failure(key)
                    .and_then(|cause| cause.downcast_ref::<std::io::Error>())
                    .map(std::io::Error::kind),
                Some(std::io::ErrorKind::TimedOut)
            );
        }
    }
    Ok(())
}
