// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exact App B-death association with genuine unopened owners cannot fabricate
//! historical Ready/source proof. Registry facts here are external boundary inputs;
//! no OS process, watch, launch ticket or VerifiedReadiness is manufactured.
use super::{recovery_retry_tests::prepared_fixture, recovery_tests::Fixture, *};

fn source(path: &str) -> Result<FrontendInstallationInput> {
    let path = std::path::PathBuf::from(path);
    Ok(FrontendInstallationInput::new(path.clone(), path.clone(), path.clone(), path)?)
}

fn successor() -> Result<(Fixture, LeaseKey, ProcessKey)> {
    let mut fixture = prepared_fixture()?;
    let binding = fixture.binding();
    let failed = fixture.leases[0];
    fixture.held.backend.policy.request(
        binding,
        ResourceRequest {
            run: binding.run,
            connection: binding.connection,
            id: RequestId::new(2)?,
            command: ResourceCommand::RetryDead(failed),
        },
    )?;
    let current = fixture
        .held
        .backend
        .policy
        .replacement_lease(binding, failed)?
        .ok_or("Actual successor interest absent")?;
    fixture.install_inert_successor(current.resource)?;
    let process = ProcessKey { id: ProcessId::new(8)?, ..fixture.process };
    let backend = fixture.held.backend.as_mut();
    backend.observe(ResourceObservation::ClaimAcquired(current.resource))?;
    backend.observe(ResourceObservation::OwnedStarted { resource: current.resource, process })?;
    backend.observe(ResourceObservation::Ready {
        resource: current.resource,
        origin: LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?,
    })?;
    Ok((fixture, current, process))
}

fn exit(fixture: &mut Fixture, current: LeaseKey, process: ProcessKey) -> Result<()> {
    fixture.held.backend.managed_exit_observation(current.resource, process, process, 3)?;
    Ok(())
}

#[test]
fn dead_successor_history_checks_current_notice_without_requiring_erased_predecessor() -> Result<()>
{
    let (mut fixture, current, process) = successor()?;
    let original = source(r"D:\SugarSubstitute-Rust\artifacts\unopened-cutoff-source")?;
    let binding = fixture.binding();
    let failed = fixture.leases[0];
    assert!(fixture.held.backend.replacement_context(binding, failed, current)?.is_none());
    assert!(fixture.held.backend.historical_replacement_metadata(binding, current).is_err());
    exit(&mut fixture, current, process)?;
    let backend = &fixture.held.backend;
    assert!(backend.policy.replacement_lease(binding, failed).is_err());
    backend.invalidated_lease(binding, current)?;
    assert!(backend.replacement_context(binding, failed, current).is_err());
    // Supplied App Ready cannot construct the private actual-VerifiedReadiness POD.
    // Current B association now succeeds without A lookup, but no metadata is invented.
    assert!(backend.historical_replacement_metadata(binding, current)?.is_none());
    assert!(backend.historical_replacement_context(binding, current, &original)?.is_none());
    assert!(backend.record(current.resource)?.managed_exit.is_some());
    Ok(())
}

#[test]
fn historical_context_rejects_foreign_binding_full_generation_and_changed_selection() -> Result<()>
{
    let (mut fixture, current, process) = successor()?;
    exit(&mut fixture, current, process)?;
    let backend = &fixture.held.backend;
    let binding = fixture.binding();
    let original = source(r"D:\SugarSubstitute-Rust\artifacts\unopened-cutoff-source")?;
    let foreign = ClientBinding { connection: ConnectionId::new(99)?, ..binding };
    assert!(backend.historical_replacement_metadata(foreign, current).is_err());
    assert!(backend.historical_replacement_context(foreign, current, &original).is_err());
    let stale = LeaseKey {
        resource: ResourceKey {
            generation: current.resource.generation.next()?,
            ..current.resource
        },
        ..current
    };
    assert!(backend.historical_replacement_metadata(binding, stale).is_err());
    assert!(backend.historical_replacement_context(binding, stale, &original).is_err());
    let changed = source(r"D:\SugarSubstitute-Rust\artifacts\changed-selection")?;
    let rejection = backend
        .historical_replacement_context(binding, current, &changed)
        .err()
        .ok_or("Changed source accepted without original comparison")?;
    assert_eq!(
        rejection.downcast_ref::<LifecycleRejection>(),
        Some(&LifecycleRejection::WrongPhase)
    );
    assert!(backend.failure(current.resource).is_some());
    Ok(())
}

#[test]
fn app_notice_without_original_host_exit_or_original_record_cannot_create_history() -> Result<()> {
    let (mut fixture, current, process) = successor()?;
    let binding = fixture.binding();
    let original = source(r"D:\SugarSubstitute-Rust\artifacts\unopened-cutoff-source")?;
    let backend = fixture.held.backend.as_mut();
    backend.observe(ResourceObservation::ManagedExited {
        resource: current.resource,
        process,
        status: 3,
        failure: LifecycleFailure::new(
            LifecycleFailureCode::UnexpectedExit,
            Box::new(std::io::Error::other("External App-only observation")),
        ),
    })?;
    backend.invalidated_lease(binding, current)?;
    assert!(backend.record(current.resource)?.managed_exit.is_none());
    assert!(backend.historical_replacement_metadata(binding, current).is_err());
    assert!(backend.historical_replacement_context(binding, current, &original).is_err());
    let removed = backend.records[1].take().ok_or("Actual original capsule absent")?;
    assert_eq!(removed.key, current.resource);
    let rejection = backend
        .historical_replacement_metadata(binding, current)
        .err()
        .ok_or("Unknown original record accepted")?;
    assert_eq!(
        rejection.downcast_ref::<LifecycleRejection>(),
        Some(&LifecycleRejection::StaleIdentity)
    );
    assert!(backend.historical_replacement_context(binding, current, &original).is_err());
    drop(removed);
    Ok(())
}

#[test]
fn independent_cleanup_failure_blocks_history_and_keeps_original_native_cause() -> Result<()> {
    let (mut fixture, current, process) = successor()?;
    exit(&mut fixture, current, process)?;
    let binding = fixture.binding();
    let original = source(r"D:\SugarSubstitute-Rust\artifacts\unopened-cutoff-source")?;
    let cause = Box::new(std::io::Error::from_raw_os_error(5));
    let pointer = std::ptr::from_ref(cause.as_ref());
    let backend = fixture.held.backend.as_mut();
    backend.records[1].as_mut().ok_or("Original successor absent")?.cleanup_failure.retain(cause);
    assert!(backend.historical_replacement_metadata(binding, current).is_err());
    assert!(backend.historical_replacement_context(binding, current, &original).is_err());
    assert_eq!(
        backend
            .cleanup_failure(current.resource)
            .and_then(|cause| cause.downcast_ref::<std::io::Error>())
            .map(std::ptr::from_ref),
        Some(pointer)
    );
    assert!(backend.failure(current.resource).is_some());
    Ok(())
}
