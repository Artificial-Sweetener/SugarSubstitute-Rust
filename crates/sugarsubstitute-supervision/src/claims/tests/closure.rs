// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove exact process-owner closure and retained cleanup through production admission.
use super::*;
use sugarsubstitute_contracts::{
    execution::{JobClass, ScopeId, ScopeSpec},
    lifecycle::{CoordinatorEpoch, InstanceId, RunGeneration, RunKey},
};

#[test]
fn binding_requires_actual_coordinator_and_rejects_repeated_binding() -> TestResult {
    let mut harness = Harness::new()?;
    assert!(matches!(
        harness.claims.bind_process_registry(&harness.registry),
        Err(ClaimAdmissionError::Identity)
    ));
    let spec = harness.spec(crate::CLAIM_POOL_BYTES)?;
    harness.claims.submit_initialize(
        &harness.client,
        &harness.scope,
        spec,
        PathBuf::from("E:\\fixture"),
    )?;
    harness.drain()?;
    let foreign = crate::WindowsProcessRegistry::new(CoordinatorKey {
        epoch: CoordinatorEpoch::new([3; 16])?,
        ..owner()?
    });
    // A different lifetime remains rejected, even after initialization and with no processes.
    assert!(matches!(
        harness.claims.bind_process_registry(&foreign),
        Err(ClaimAdmissionError::Registry(crate::RegistryAdmissionError::ForeignOwner))
    ));
    harness.claims.bind_process_registry(&harness.registry)?;
    assert!(matches!(
        harness.claims.bind_process_registry(&harness.registry),
        Err(ClaimAdmissionError::Identity)
    ));
    harness.close(0)?;
    Ok(())
}

#[test]
fn desktop_only_close_requires_pinned_registry_and_seals_before_empty_proof() -> TestResult {
    let mut harness = Harness::new()?;
    harness.initialize()?;
    let mut foreign = crate::WindowsProcessRegistry::new(owner()?);
    harness.claims.close_admission();
    let spec = harness.spec(CLAIM_OPERATION_BYTES)?;
    assert!(matches!(
        harness.claims.submit_close(&harness.client, &harness.scope, spec, &mut foreign),
        Err(ClaimAdmissionError::Identity)
    ));
    assert_eq!(harness.scope.snapshot().reserved_bytes, crate::CLAIM_POOL_BYTES);

    let cancelled =
        harness.client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
    cancelled.cancel();
    let command = crate::WindowsLaunchCommand::from_utf16(
        &"E:\\fixture\\child.exe".encode_utf16().collect::<Vec<_>>(),
        &"E:\\fixture".encode_utf16().collect::<Vec<_>>(),
        &[],
    )?;
    let input = crate::WindowsLaunchInput::new(
        command,
        crate::LaunchEnvironment::from_snapshot_utf16(&[0, 0], &[])?,
    )?;
    let mut watch = harness.spec(4096)?;
    watch.class = JobClass::NativeObservation;
    let creation = harness.spec(input.required_reservation_bytes())?;
    let binding = crate::FamilyBinding::Desktop(RunKey {
        owner: owner()?,
        instance: InstanceId::new(1)?,
        generation: RunGeneration::new(1)?,
    });
    assert!(matches!(
        harness.registry.submit_launch(
            &harness.client,
            &cancelled,
            ScopeSpec { id: ScopeId::new(3)?, deadline: None },
            creation,
            watch,
            binding,
            input
        ),
        Err(crate::RegistryAdmissionError::Execution(_))
    ));
    assert!(!harness.registry.is_empty());
    let spec = harness.spec(CLAIM_OPERATION_BYTES)?;
    assert!(matches!(
        harness.claims.submit_close(&harness.client, &harness.scope, spec, &mut harness.registry),
        Err(ClaimAdmissionError::Busy)
    ));
    harness.drain_registry()?;
    let command = crate::WindowsLaunchCommand::from_utf16(
        &"E:\\fixture\\child.exe".encode_utf16().collect::<Vec<_>>(),
        &"E:\\fixture".encode_utf16().collect::<Vec<_>>(),
        &[],
    )?;
    let input = crate::WindowsLaunchInput::new(
        command,
        crate::LaunchEnvironment::from_snapshot_utf16(&[0, 0], &[])?,
    )?;
    let mut watch = harness.spec(4096)?;
    watch.class = JobClass::NativeObservation;
    let creation = harness.spec(input.required_reservation_bytes())?;
    assert!(matches!(
        harness.registry.submit_launch(
            &harness.client,
            &harness.scope,
            ScopeSpec { id: ScopeId::new(4)?, deadline: None },
            creation,
            watch,
            binding,
            input
        ),
        Err(crate::RegistryAdmissionError::Closed)
    ));
    harness.close(0)?;
    assert_eq!(harness.scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn failed_native_close_preserves_actual_pool_until_checked_retry() -> TestResult {
    let mut harness = Harness::new()?;
    harness.initialize()?;
    *lock(&harness.fake.failure) = Some(Stage::Close);
    harness.close(crate::CLAIM_POOL_BYTES)?;
    assert!(!harness.claims.closed);
    assert_eq!(harness.scope.snapshot().reserved_bytes, crate::CLAIM_POOL_BYTES);
    let mut failed = false;
    while let Some(event) = harness.claims.take_observation() {
        assert!(!matches!(event, ClaimObservation::NativeClaimsClosed));
        failed |= matches!(event, ClaimObservation::Failed { .. });
    }
    assert!(failed);
    harness.close(0)?;
    assert!(harness.claims.closed);
    assert_eq!(harness.scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn worker_panic_does_not_discard_actual_native_acquisition_or_pool() -> TestResult {
    let mut harness = Harness::new()?;
    *lock(&harness.fake.panic_at) = Some(Stage::Initialize);
    let spec = harness.spec(crate::CLAIM_POOL_BYTES)?;
    harness.claims.submit_initialize(
        &harness.client,
        &harness.scope,
        spec,
        PathBuf::from("E:\\fixture"),
    )?;
    harness.drain()?;
    assert!(harness.claims.initialized);
    assert_eq!(harness.scope.snapshot().reserved_bytes, crate::CLAIM_POOL_BYTES);
    let mut failed = false;
    while let Some(event) = harness.claims.take_observation() {
        failed |= matches!(event, ClaimObservation::Failed { ref cause, .. } if cause.operation() == "claim_worker_panic");
    }
    assert!(failed);
    harness.close(0)?;
    assert_eq!(harness.scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn unattempted_backend_release_rejects_a_different_empty_registry() -> TestResult {
    let mut harness = Harness::new()?;
    harness.initialize()?;
    let ticket = harness.resolve()?;
    let resource = harness.resource(1)?;
    let spec = harness.spec(CLAIM_OPERATION_BYTES)?;
    harness.claims.submit_backend_claim(
        &harness.client,
        &harness.scope,
        spec,
        resource,
        &ticket,
    )?;
    harness.drain()?;
    let foreign = crate::WindowsProcessRegistry::new(owner()?);
    let spec = harness.spec(CLAIM_OPERATION_BYTES)?;
    assert!(matches!(
        harness.claims.submit_backend_release(
            &harness.client,
            &harness.scope,
            spec,
            &foreign,
            resource
        ),
        Err(ClaimAdmissionError::Identity)
    ));
    let spec = harness.spec(CLAIM_OPERATION_BYTES)?;
    harness.claims.submit_backend_release(
        &harness.client,
        &harness.scope,
        spec,
        &harness.registry,
        resource,
    )?;
    harness.drain()?;
    harness.close(0)?;
    Ok(())
}

#[test]
fn partial_initialization_cleanup_never_seals_an_unowned_registry() -> TestResult {
    let mut harness = Harness::new()?;
    harness.claims.close_admission();
    let spec = harness.spec(CLAIM_OPERATION_BYTES)?;
    harness.claims.submit_close(&harness.client, &harness.scope, spec, &mut harness.registry)?;
    harness.arm_memory_release(0)?;
    harness.drain()?;
    harness.wait_reserved(&harness.scope, 0)?;
    let cancelled =
        harness.client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
    cancelled.cancel();
    let command = crate::WindowsLaunchCommand::from_utf16(
        &"E:\\fixture\\child.exe".encode_utf16().collect::<Vec<_>>(),
        &"E:\\fixture".encode_utf16().collect::<Vec<_>>(),
        &[],
    )?;
    let input = crate::WindowsLaunchInput::new(
        command,
        crate::LaunchEnvironment::from_snapshot_utf16(&[0, 0], &[])?,
    )?;
    let mut watch = harness.spec(4096)?;
    watch.class = JobClass::NativeObservation;
    let creation = harness.spec(input.required_reservation_bytes())?;
    let binding = crate::FamilyBinding::Desktop(RunKey {
        owner: owner()?,
        instance: InstanceId::new(1)?,
        generation: RunGeneration::new(1)?,
    });
    assert!(matches!(
        harness.registry.submit_launch(
            &harness.client,
            &cancelled,
            ScopeSpec { id: ScopeId::new(3)?, deadline: None },
            creation,
            watch,
            binding,
            input
        ),
        Err(crate::RegistryAdmissionError::Execution(_))
    ));
    assert!(!harness.registry.is_empty());
    harness.drain_registry()?;
    Ok(())
}
