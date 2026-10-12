// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind rejected creation cleanup to the real native-observation retirement owner.
use super::*;
use crate::{ClaimLaunchJobs, LaunchEnvironment};
use sugarsubstitute_contracts::execution::{ExecutionPort, JobClass, ScopeId, ScopeSpec};

#[test]
fn rejected_managed_attempt_requires_exact_registry_and_observed_watch_retirement() -> TestResult {
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
    let input = harness
        .claims
        .prepare_managed_launch(resource, LaunchEnvironment::from_snapshot_utf16(&[0, 0], &[])?)?;
    let creation_scope =
        harness.client.create_scope(ScopeSpec { id: ScopeId::new(3)?, deadline: None }, None)?;
    creation_scope.cancel();
    let mut watch = harness.spec(4096)?;
    watch.class = JobClass::NativeObservation;
    let creation = harness.spec(input.required_reservation_bytes())?;
    let jobs = ClaimLaunchJobs {
        native_scope: ScopeSpec { id: ScopeId::new(4)?, deadline: None },
        creation,
        root_watch: watch,
    };
    assert!(matches!(
        harness.claims.submit_managed_launch(
            &harness.client,
            &mut harness.registry,
            &creation_scope,
            jobs,
            input
        ),
        Err(ClaimAdmissionError::Registry(_))
    ));
    assert!(harness.registry.retained_managed(resource)?.is_some());
    let spec = harness.spec(CLAIM_OPERATION_BYTES)?;
    assert!(matches!(
        harness.claims.submit_backend_release(
            &harness.client,
            &harness.scope,
            spec,
            &harness.registry,
            resource
        ),
        Err(ClaimAdmissionError::Busy)
    ));
    let foreign = harness.construct_registry(owner()?)?;
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
    harness.drain_registry()?;
    assert!(harness.registry.retained_managed(resource)?.is_none());
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
fn same_coordinator_facts_do_not_allow_another_executor_to_consume_claim_operations() -> TestResult
{
    let mut first = Harness::new()?;
    first.initialize()?;
    let executor = sugarsubstitute_execution::ExecutionOwner::new(
        Arc::new(ClockPort::default()),
        Arc::new(Wake::default()),
    )?;
    let client = executor.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let spec = first.spec(CLAIM_OPERATION_BYTES)?;
    assert!(matches!(
        first.claims.submit_resolve_launch(&client, &scope, spec, configuration()?),
        Err(ClaimAdmissionError::Identity)
    ));
    assert_eq!(first.fake.resolutions.load(Ordering::Acquire), 0);
    first.close(0)?;
    Ok(())
}
