// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose real policy Pending leases and real output jobs; authenticate no peer or process.
use super::*;
use sugarsubstitute_execution::windows_output::{OutputIdentity, boundary_fixture::OutputFixture};

/// Admit genuine policy Pending leases and the same Core output worker over an external fake.
pub(in crate::production) fn shared(
    client: &ExecutionClient,
    native: &Scope,
    external: &OutputFixture,
) -> Result<(RetainedBackend, [LeaseKey; 3])> {
    let mut held = tests::unadmitted_policy()?;
    let backend = held.backend.as_mut();
    let owner = backend.owner;
    let root = std::env::current_dir()?;
    let target =
        BackendTarget::Managed(sugarsubstitute_contracts::configuration::LaunchConfiguration::new(
            root.clone(),
            root.join("unopened-output-python.exe"),
            vec![],
            8188,
        )?);
    let mut leases = Vec::new();
    for instance in 1..=3 {
        let run = RunKey {
            owner,
            instance: InstanceId::new(instance)?,
            generation: RunGeneration::new(1)?,
        };
        let connection = ConnectionId::new(instance)?;
        let registration = ClientRegistration {
            run,
            connection,
            launch: sugarsubstitute_contracts::startup::LaunchId::new(1)?,
            profile: sugarsubstitute_contracts::configuration::ProfileId::new(format!(
                "Fanout_{instance}"
            ))?,
        };
        backend.policy.enroll_run(RunEnrollment {
            run,
            launch: registration.launch,
            profile: registration.profile.clone(),
        })?;
        backend.register_run(registration)?;
        let transition = backend.policy.request(
            ClientBinding { run, connection },
            ResourceRequest {
                run,
                connection,
                id: RequestId::new(1)?,
                command: ResourceCommand::Acquire(target.clone()),
            },
        )?;
        let Some(RequestOutcome::Pending(lease)) =
            transition.replies.first().map(|reply| reply.outcome)
        else {
            return Err("Actual shared Pending reservation absent".into());
        };
        leases.push(lease);
    }
    let leases: [LeaseKey; 3] = leases.try_into().map_err(|_| LifecycleRejection::WrongPhase)?;
    if leases.iter().any(|lease| lease.resource != leases[0].resource) {
        return Err(LifecycleRejection::StaleIdentity.into());
    }
    let scope = client.create_scope(
        ScopeSpec { id: sugarsubstitute_contracts::execution::ScopeId::new(2)?, deadline: None },
        Some(native),
    )?;
    let mut record = cutoff_tests::creator(
        leases[0].resource,
        target,
        ConnectionBudget::new(ConnectionBudgetKind::Managed, client.now()?)?,
        scope,
    )?;
    record.output = Some(external.reserve(
        client,
        native,
        JobSpec {
            id: sugarsubstitute_contracts::execution::JobId::new(1)?,
            operation: Operation::NativeLifecycle,
            class: JobClass::NativeObservation,
            deadline: None,
            replacement: None,
            reservation_bytes: MIN_WINDOWS_OUTPUT_BYTES,
        },
        OutputIdentity::Managed(leases[0].resource),
    )?);
    backend.records[0] = Some(record);
    Ok((held, leases))
}
/// Inspect only actual native output state, without creating a batch or retirement fact.
pub(in crate::production) fn snapshot(
    backend: &CoordinatorBackend,
    key: ResourceKey,
) -> Result<WindowsOutputReport> {
    Ok(backend
        .record(key)?
        .output
        .as_ref()
        .ok_or(LifecycleRejection::WrongPhase)?
        .control
        .snapshot())
}
/// The same accepted policy disconnect precedes the production subscription release path.
pub(in crate::production) fn disconnect(
    backend: &mut CoordinatorBackend,
    lease: LeaseKey,
) -> Result<()> {
    backend.policy.disconnect_run(lease.run, lease.connection)?;
    backend.output_clients.remove_client(lease.run, lease.connection);
    backend.release_consumed_output();
    Ok(())
}
/// Adopt the real pipe worker terminal before allowing the fixture ExecutionOwner to join.
pub(in crate::production) fn retire(
    backend: &mut CoordinatorBackend,
    key: ResourceKey,
) -> Result<bool> {
    let at = backend.index(key)?;
    let record = backend.records[at].as_mut().ok_or(LifecycleRejection::StaleIdentity)?;
    let output = record.output.as_mut().ok_or(LifecycleRejection::WrongPhase)?;
    output.control.close();
    if !record.output_observed
        && let Some(terminal) = output.completion.try_take_completion()
    {
        record.output_observed = true;
        record.output_terminal = Some(terminal);
    }
    Ok(record.output_observed && output.control.snapshot().retired)
}

/// Exercise actual global cleanup while a Parent still owns its copied, unacknowledged delivery.
pub(in crate::production) fn close(
    backend: &mut CoordinatorBackend,
    key: ResourceKey,
    client: &ExecutionClient,
    native: &Scope,
) -> Result<()> {
    let cleanup = client.create_scope(
        ScopeSpec { id: sugarsubstitute_contracts::execution::ScopeId::new(3)?, deadline: None },
        None,
    )?;
    let jobs = JobSequence::new();
    let scopes = ScopeSequence::new();
    let mut authority = CoordinatorAuthority::new(
        sugarsubstitute_contracts::coordinator_entry::EntrySetupBudget::new(client.now()?)?,
    )?;
    let original = ShutdownBudget::new(client.now()?)?;
    let mut resources = BackendResources {
        client,
        native,
        cleanup: &cleanup,
        jobs: &jobs,
        scopes: &scopes,
        authority: &mut authority,
        capture: None,
    };
    backend.begin_cleanup(key, Some(original), &mut resources)?;
    let record = backend.record(key)?;
    assert_eq!(record.shutdown.map(|(_, budget)| budget), Some(original));
    assert!(record.output_fanout.current.is_none());
    Ok(())
}
