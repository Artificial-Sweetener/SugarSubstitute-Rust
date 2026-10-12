// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise claim ownership through real admitted jobs and a synchronized native boundary.
mod cancellation;
mod closure;
mod entry;
mod managed;
mod support;
use super::*;
use support::*;
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn retained_claim_observations_fit_the_native_composition_stack_budget() -> TestResult {
    assert!(std::mem::size_of::<WindowsResourceClaims>() <= 32 * 1024);
    let first = WindowsResourceClaims::new()?;
    let second = WindowsResourceClaims::new()?;
    assert_eq!(first.events.entries.len(), 512);
    assert_eq!(second.events.entries.len(), 512);
    assert!(first.events.entries.iter().all(Option::is_none));
    assert!(!std::ptr::eq(first.events.entries.as_ptr(), second.events.entries.as_ptr()));
    Ok(())
}

#[test]
fn diagnostics_redact_private_causes_and_preserve_source() {
    use std::error::Error;
    let cause = ClaimError::new("fixture", std::io::Error::other("private-path-secret"));
    assert!(!format!("{cause:?} {cause}").contains("private-path-secret"));
    assert!(cause.source().is_some_and(|source| source.to_string() == "private-path-secret"));
    assert!(!format!("{:?}", ClaimAdmissionError::Native(cause)).contains("private-path-secret"));
}

#[test]
fn checked_cleanup_preserves_primary_and_every_secondary_without_rendering_secrets() -> TestResult {
    use std::error::Error;
    let primary = ClaimError::new("primary", std::io::Error::other("primary-secret"));
    let first = ClaimError::new("first_cleanup", std::io::Error::other("first-secret"));
    let second = ClaimError::new("second_cleanup", std::io::Error::other("second-secret"));
    let result: Result<(), ClaimError> = ClaimError::finish(Err(primary), Err(first));
    let cause = ClaimError::finish(result, Err(second)).err().ok_or("Missing first failure")?;
    assert_eq!(cause.operation(), "primary");
    assert!(cause.source().is_some_and(|source| source.to_string() == "primary-secret"));
    let cleanup = cause.cleanup_cause().ok_or("Missing first cleanup")?;
    assert_eq!(cleanup.operation(), "first_cleanup");
    assert_eq!(
        cleanup.cleanup_cause().ok_or("Missing second cleanup")?.operation(),
        "second_cleanup"
    );
    assert!(!format!("{cause} {cause:?} {cleanup} {cleanup:?}").contains("secret"));
    Ok(())
}

#[test]
fn oversized_configuration_is_rejected_before_cloning_or_slot_admission() -> TestResult {
    let mut harness = Harness::new()?;
    harness.initialize()?;
    let root = PathBuf::from(format!("E:\\{}", "x".repeat(32_763)));
    let interpreter = PathBuf::from(format!("E:\\{}", "y".repeat(32_763)));
    let arguments = vec!["a".repeat(4096); 16];
    let configuration = LaunchConfiguration::new(root, interpreter, arguments, 8188)?;
    let spec = harness.spec(CLAIM_OPERATION_BYTES)?;
    assert!(matches!(
        harness.claims.submit_resolve_launch(&harness.client, &harness.scope, spec, configuration),
        Err(ClaimAdmissionError::Specification)
    ));
    assert_eq!(harness.fake.resolutions.load(Ordering::Acquire), 0);
    assert!(harness.claims.resolutions.iter().all(Option::is_none));
    assert_eq!(harness.scope.snapshot().reserved_bytes, crate::CLAIM_POOL_BYTES);
    harness.close(0)?;
    Ok(())
}

#[test]
fn initialization_acknowledges_job_credit_but_retains_actual_pool_bytes() -> TestResult {
    let mut harness = Harness::new()?;
    harness.initialize()?;
    assert_eq!(harness.scope.snapshot().terminal_unobserved, 0);
    assert_eq!(harness.scope.snapshot().reserved_bytes, crate::CLAIM_POOL_BYTES);
    let pool = lock(&harness.claims.root.value).pool.clone().ok_or("Missing actual pool")?;
    let mut uses = Vec::new();
    for _ in 0..128 {
        uses.push(pool.acquire().ok_or("Missing bounded slot")?);
    }
    assert!(pool.acquire().is_none());
    drop(uses);
    assert!(pool.acquire().is_some());
    drop(pool);
    harness.close(0)?;
    assert_eq!(harness.scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn exact_resolution_ticket_cannot_be_recreated_by_another_claim_owner() -> TestResult {
    let mut harness = Harness::new()?;
    harness.initialize()?;
    let ticket = harness.resolve()?;
    assert!(harness.claims.resolved_configuration(&ticket).is_ok());
    let other = WindowsResourceClaims::new()?;
    assert!(matches!(other.resolved_configuration(&ticket), Err(ClaimAdmissionError::Identity)));
    harness.retire(&ticket)?;
    harness.close(0)?;
    Ok(())
}

#[test]
fn backend_release_closes_only_the_unlaunched_current_generation() -> TestResult {
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
    assert!(harness.claims.resolved_configuration(&ticket).is_err());
    let input = harness.claims.prepare_managed_launch(
        resource,
        crate::LaunchEnvironment::from_snapshot_utf16(&[0, 0], &[])?,
    )?;
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
    drop(input);
    let spec = harness.spec(CLAIM_OPERATION_BYTES)?;
    harness.claims.submit_backend_release(
        &harness.client,
        &harness.scope,
        spec,
        &harness.registry,
        resource,
    )?;
    harness.drain()?;
    assert!(harness.claims.backends.iter().all(Option::is_none));
    harness.close(0)?;
    Ok(())
}

#[test]
fn resolution_admission_is_bounded_before_the_native_factory() -> TestResult {
    let mut harness = Harness::new()?;
    harness.initialize()?;
    let mut tickets = Vec::new();
    for _ in 0..64 {
        tickets.push(harness.resolve()?);
    }
    let spec = harness.spec(CLAIM_OPERATION_BYTES)?;
    assert!(matches!(
        harness.claims.submit_resolve_launch(
            &harness.client,
            &harness.scope,
            spec,
            configuration()?
        ),
        Err(ClaimAdmissionError::Saturated)
    ));
    assert_eq!(harness.fake.resolutions.load(Ordering::Acquire), 64);
    for ticket in tickets {
        harness.retire(&ticket)?;
    }
    harness.close(0)?;
    Ok(())
}
