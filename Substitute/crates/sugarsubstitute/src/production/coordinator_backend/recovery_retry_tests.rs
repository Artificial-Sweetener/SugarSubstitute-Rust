// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain exact source lineage, cohort returns and original finite retry inputs.
//! Supplied policy observations do not construct a verified Ready/native proof.
use super::{recovery_tests::Fixture, *};

#[test]
fn exact_retry_replay_uses_original_app_reply_without_new_source_or_campaign() -> Result<()> {
    let mut fixture = prepared_fixture()?;
    let binding = fixture.binding();
    let failed = fixture.leases[0];
    let request = ResourceRequest {
        run: binding.run,
        connection: binding.connection,
        id: RequestId::new(2)?,
        command: ResourceCommand::RetryDead(failed),
    };
    let original = input(r"D:\SugarSubstitute-Rust\artifacts\unopened-cutoff-source")?;
    let budget = ConnectionBudget::new(ConnectionBudgetKind::Managed, Moment::from_nanos(0))?;
    let backend = fixture.held.backend.as_mut();
    assert_eq!(
        backend.request_preparation(
            binding,
            &request,
            Some(budget),
            Some(&original),
            Moment::from_nanos(0)
        )?,
        Some(failed.resource)
    );
    let first = backend.policy.request(binding, request.clone())?;
    assert_eq!(first.effects.len(), 1);
    assert!(backend.policy.request_replay(binding, &request)?);
    assert_eq!(backend.request_preparation(binding, &request, None, None, budget.until())?, None);
    let replay = backend.policy.request(binding, request.clone())?;
    assert_eq!(replay.replies, first.replies);
    assert!(replay.effects.is_empty());
    let fresh = ResourceRequest { id: RequestId::new(3)?, ..request.clone() };
    assert!(
        backend
            .request_preparation(
                binding,
                &fresh,
                Some(budget),
                Some(&original),
                Moment::from_nanos(0)
            )
            .is_err()
    );
    let changed = ResourceRequest { command: ResourceCommand::Release(failed), ..request };
    assert!(backend.policy.request_replay(binding, &changed).is_err());
    Ok(())
}

fn input(path: &str) -> Result<BackendPreparation> {
    let path = std::path::PathBuf::from(path);
    Ok(BackendPreparation {
        source: FrontendInstallationInput::new(path.clone(), path.clone(), path.clone(), path)?,
        environment: Some(LaunchEnvironment::from_snapshot_utf16(&[0, 0], &[])?),
    })
}

pub(super) fn prepared_fixture() -> Result<Fixture> {
    let mut fixture = Fixture::new(true)?;
    fixture.exit()?;
    fixture.retire_inert()?;
    let mut cause = None;
    assert!(
        fixture.held.backend.transfer_recovery_failure(fixture.leases[0].resource, &mut cause)?
    );
    assert_eq!(
        cause.as_ref().ok_or("Actual cause absent")?.code(),
        LifecycleFailureCode::UnexpectedExit
    );
    assert!(fixture.held.backend.take_pending_transition().is_some());
    Ok(fixture)
}

#[test]
fn retry_selection_rejects_changed_source_environment_or_binding_before_policy_mutation()
-> Result<()> {
    let fixture = prepared_fixture()?;
    let backend = &fixture.held.backend;
    let failed = fixture.leases[0];
    let binding = fixture.binding();
    let original = input(r"D:\SugarSubstitute-Rust\artifacts\unopened-cutoff-source")?;
    assert_eq!(backend.retry_preparation(binding, failed, Some(&original))?, failed.resource);
    let changed = input(r"D:\SugarSubstitute-Rust\artifacts\unopened-other-source")?;
    assert!(backend.retry_preparation(binding, failed, Some(&changed)).is_err());
    let no_environment =
        BackendPreparation { source: original.source.try_reissue()?, environment: None };
    assert!(backend.retry_preparation(binding, failed, Some(&no_environment)).is_err());
    assert!(backend.retry_preparation(binding, failed, None).is_err());
    let foreign = ClientBinding { connection: ConnectionId::new(99)?, ..binding };
    assert!(backend.retry_preparation(foreign, failed, Some(&original)).is_err());
    assert_eq!(backend.policy.replacement_lease(binding, failed)?, None);
    assert_eq!(
        backend.invalidated_lease(binding, failed)?,
        sugarsubstitute_application::supervision::InvalidatedLeaseState::RetryReady
    );
    Ok(())
}

#[test]
fn failed_pre_record_successor_reuses_only_retained_same_resource_original_source() -> Result<()> {
    let mut fixture = prepared_fixture()?;
    let binding = fixture.binding();
    let failed = fixture.leases[0];
    let backend = fixture.held.backend.as_mut();
    let next = backend.policy.request(
        binding,
        ResourceRequest {
            run: binding.run,
            connection: binding.connection,
            id: RequestId::new(2)?,
            command: ResourceCommand::RetryDead(failed),
        },
    )?;
    let Some(RequestOutcome::Pending(current)) = next.replies.first().map(|reply| reply.outcome)
    else {
        return Err("Actual retry admission absent".into());
    };
    assert!(backend.record(current.resource).is_err());
    assert!(backend.policy.retains_outage_resource(failed.resource.id));
    backend.observe(ResourceObservation::Failed {
        resource: current.resource,
        failure: LifecycleFailure::new(
            LifecycleFailureCode::Preparation,
            Box::new(std::io::Error::other("External pre-record admission rejected")),
        ),
    })?;
    backend.observe(ResourceObservation::Settled(current.resource))?;
    let mut cause = None;
    assert!(backend.transfer_recovery_failure(current.resource, &mut cause)?);
    assert_eq!(
        cause.as_ref().ok_or("Admission cause absent")?.code(),
        LifecycleFailureCode::Preparation
    );
    let original = input(r"D:\SugarSubstitute-Rust\artifacts\unopened-cutoff-source")?;
    assert_eq!(backend.retry_preparation(binding, failed, Some(&original))?, failed.resource);
    let foreign = ResourceKey { id: ResourceId::new(99)?, ..current.resource };
    assert!(backend.retained_source(foreign).is_err());
    assert_eq!(backend.records.iter().flatten().count(), 1);
    Ok(())
}

#[test]
fn actual_policy_replacements_keep_every_cohort_return_and_adoption_is_per_connection() -> Result<()>
{
    let mut fixture = prepared_fixture()?;
    let binding = fixture.binding();
    let leases = fixture.leases;
    let process = fixture.process;
    let backend = fixture.held.backend.as_mut();
    let request = backend.policy.request(
        binding,
        ResourceRequest {
            run: binding.run,
            connection: binding.connection,
            id: RequestId::new(2)?,
            command: ResourceCommand::RetryDead(leases[0]),
        },
    )?;
    assert_eq!(request.effects.len(), 1);
    let current = backend
        .policy
        .replacement_lease(binding, leases[0])?
        .ok_or("Original retry interest absent")?;
    backend.begin_delivery()?;
    backend.observe(ResourceObservation::ClaimAcquired(current.resource))?;
    backend.observe(ResourceObservation::OwnedStarted {
        resource: current.resource,
        process: ProcessKey { id: ProcessId::new(8)?, ..process },
    })?;
    let ready = backend.observe(ResourceObservation::Ready {
        resource: current.resource,
        origin: LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?,
    })?;
    assert_eq!(ready.replacements.len(), 2);
    let pending = backend.take_pending_transition().ok_or("Whole producer return absent")?;
    assert_eq!(pending.replacements, ready.replacements);
    for pair in &ready.replacements {
        assert!(leases.contains(&pair.previous));
        assert_eq!(pair.current.run, pair.previous.run);
        assert_eq!(pair.current.connection, pair.previous.connection);
        assert_eq!(pair.current.resource, current.resource);
    }
    let first = ready
        .replacements
        .iter()
        .find(|pair| pair.previous == leases[0])
        .ok_or("First cohort transition absent")?;
    backend.adopt_replacement(binding, first.previous, first.current)?;
    assert!(backend.invalidated_lease(binding, leases[0]).is_err());
    let other = ClientBinding { run: leases[1].run, connection: leases[1].connection };
    assert!(backend.policy.replacement_lease(other, leases[1])?.is_some());
    // These supplied App facts cannot create actual installed/readiness metadata.
    // The genuine historical context accessor must not promote them.
    assert!(backend.record(current.resource).is_err());
    Ok(())
}

#[test]
fn original_funded_policy_partition_rejects_before_observation_and_retains_original_cause()
-> Result<()> {
    let mut fixture = Fixture::new(true)?;
    let key = fixture.leases[0].resource;
    fixture.held.backend.policy_funding.limit_for_test(0)?;
    let cause = Box::new(std::io::Error::other("Original external root observation"));
    let pointer = std::ptr::from_ref(cause.as_ref());
    let rejected = fixture
        .held
        .backend
        .observe(ResourceObservation::ManagedExited {
            resource: key,
            process: fixture.process,
            status: 4,
            failure: LifecycleFailure::new(LifecycleFailureCode::UnexpectedExit, cause),
        })
        .err()
        .ok_or("Unfunded policy mutation accepted")?;
    let retained = rejected
        .source()
        .and_then(Error::source)
        .and_then(|cause| cause.downcast_ref::<std::io::Error>())
        .ok_or("Original cause lost in admission rejection")?;
    assert_eq!(std::ptr::from_ref(retained), pointer);
    assert!(fixture.held.backend.failure(key).is_none());
    assert!(fixture.held.backend.invalidated_lease(fixture.binding(), fixture.leases[0]).is_err());
    Ok(())
}
