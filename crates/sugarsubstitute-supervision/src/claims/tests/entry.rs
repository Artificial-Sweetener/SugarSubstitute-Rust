// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind Serving only from real consumed entry and claims lifetimes over existing external fakes.
use super::*;
use crate::coordinator_entry::{EntryPhase, fixture::Harness as EntryHarness};
use sugarsubstitute_contracts::execution::Moment;

fn claims(
    entry: &EntryHarness,
    fake: Arc<Fake>,
) -> Result<WindowsResourceClaims, Box<dyn std::error::Error>> {
    let mut claims = WindowsResourceClaims::new()?;
    claims.port = fake;
    entry.owner.prepare_claims(&mut claims)?;
    Ok(claims)
}
fn initialization_spec(entry: &mut EntryHarness) -> Result<JobSpec, Box<dyn std::error::Error>> {
    let mut spec = entry.setup_spec()?;
    spec.reservation_bytes = crate::CLAIM_POOL_BYTES;
    Ok(spec)
}
fn retire(
    entry: &mut EntryHarness,
    claims: &mut WindowsResourceClaims,
    registry: &mut crate::WindowsProcessRegistry,
) -> TestResult {
    claims.close_admission();
    if claims.registry.is_none() {
        claims.bind_process_registry(registry)?;
    }
    let mut spec = entry.spec(Moment::from_nanos(40_000_000_000))?;
    spec.reservation_bytes = CLAIM_OPERATION_BYTES;
    let expected = claims.submit_close(&entry.client, &entry.scope, spec, registry)?;
    entry.consume_claims(claims, expected)?;
    assert!(claims.closed);
    assert!(lock(&claims.root.value).pool.is_none());
    entry.close(Moment::from_nanos(40_000_000_000))?;
    Ok(())
}

#[test]
fn entry_serving_requires_consumed_claim_success_and_exact_registry_then_obeys_claim_close()
-> TestResult {
    let mut entry = EntryHarness::new()?;
    entry.discover()?;
    entry.elect()?;
    let fake = Arc::new(Fake::default());
    let gate = Arc::new(Barrier::default());
    *lock(&fake.gate) = Some((Stage::Initialize, gate.clone()));
    let mut claims = claims(&entry, fake)?;
    let mut registry = construct_registry(&mut entry)?;
    let spec = initialization_spec(&mut entry)?;
    let expected = claims.submit_initialize(
        &entry.client,
        &entry.scope,
        spec,
        PathBuf::from(r"E:\fixture"),
    )?;
    gate.wait(1)?;
    assert!(entry.owner.bind_coordinator(&claims, &registry, owner()?).is_err());
    assert_eq!(entry.owner.snapshot().phase, EntryPhase::ElectionHeld);
    gate.release();
    entry.consume_claims(&mut claims, expected)?;
    assert!(claims.entry_initialized_held.load(Ordering::Acquire));
    claims.bind_process_registry(&registry)?;
    let foreign = construct_registry(&mut entry)?;
    assert!(entry.owner.bind_coordinator(&claims, &foreign, owner()?).is_err());
    let uninitialized = WindowsResourceClaims::new()?;
    assert!(entry.owner.bind_coordinator(&uninitialized, &registry, owner()?).is_err());
    entry.owner.bind_coordinator(&claims, &registry, owner()?)?;
    assert_eq!(entry.owner.snapshot().phase, EntryPhase::Serving);
    let retained = claims.retained_coordinator(&registry, owner()?, entry.scope.owner_id())?;
    retained.validate_live(owner()?, entry.scope.owner_id())?;
    claims.close_admission();
    assert!(retained.validate_live(owner()?, entry.scope.owner_id()).is_err());
    retire(&mut entry, &mut claims, &mut registry)
}

#[test]
fn failed_or_late_cancelled_claim_acquisition_retains_custody_and_never_grants_serving()
-> TestResult {
    for failed in [false, true] {
        let mut entry = EntryHarness::new()?;
        entry.discover()?;
        entry.elect()?;
        let fake = Arc::new(Fake::default());
        let gate = Arc::new(Barrier::default());
        *lock(&fake.gate) = Some((Stage::Initialize, gate.clone()));
        if failed {
            *lock(&fake.failure) = Some(Stage::Initialize);
        }
        let mut claims = claims(&entry, fake)?;
        let mut registry = construct_registry(&mut entry)?;
        let spec = initialization_spec(&mut entry)?;
        let expected = claims.submit_initialize(
            &entry.client,
            &entry.scope,
            spec,
            PathBuf::from(r"E:\fixture"),
        )?;
        gate.wait(1)?;
        if !failed {
            claims.close_admission();
            claims
                .pending
                .iter()
                .flatten()
                .next()
                .ok_or("initialization job missing")?
                .job
                .cancel();
        }
        gate.release();
        entry.consume_claims(&mut claims, expected)?;
        assert!(claims.initialized);
        assert!(!claims.entry_initialized_held.load(Ordering::Acquire));
        assert!(lock(&claims.root.value).pool.is_some());
        assert!(entry.owner.bind_coordinator(&claims, &registry, owner()?).is_err());
        assert_eq!(entry.owner.snapshot().phase, EntryPhase::ElectionHeld);
        retire(&mut entry, &mut claims, &mut registry)?;
    }
    Ok(())
}

/// Use the existing entry owner/counter/wake to observe admitted registry construction.
fn construct_registry(
    entry: &mut EntryHarness,
) -> Result<crate::WindowsProcessRegistry, Box<dyn std::error::Error>> {
    let mut spec = entry.setup_spec()?;
    spec.reservation_bytes = crate::WindowsProcessRegistry::required_reservation_bytes();
    let mut tables = Vec::new();
    crate::registry::construction_fixture::construct(
        &entry.client,
        &entry.scope,
        spec,
        owner()?,
        entry.wake.as_ref(),
        &mut tables,
        |batch| {
            entry.owner.consume(batch)?;
            Ok(())
        },
    )
}
