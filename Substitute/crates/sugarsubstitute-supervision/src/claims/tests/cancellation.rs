// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Synchronize cancellation on actual worker entry and retained native acquisition.
use super::*;
use sugarsubstitute_contracts::execution::{ExecutionPort, TerminalKind};

#[test]
fn cancelled_initialization_before_factory_has_no_native_effects_or_retained_pool() -> TestResult {
    let mut harness = Harness::new()?;
    let barrier = Arc::new(Barrier::default());
    let first_spec = harness.spec(4096)?;
    let second_spec = harness.spec(4096)?;
    let first_gate = barrier.clone();
    let second_gate = barrier.clone();
    let mut first =
        harness.client.submit_blocking(&harness.scope, first_spec, move |_| first_gate.enter())?;
    let mut second = harness
        .client
        .submit_blocking(&harness.scope, second_spec, move |_| second_gate.enter())?;
    barrier.wait(2)?;
    let spec = harness.spec(crate::CLAIM_POOL_BYTES)?;
    harness.claims.submit_initialize(
        &harness.client,
        &harness.scope,
        spec,
        PathBuf::from("E:\\fixture"),
    )?;
    harness.scope.cancel();
    barrier.release();
    harness.drain()?;
    let first_completion = harness.completion(&mut first)?;
    let second_completion = harness.completion(&mut second)?;
    let original = harness.fresh_scope()?;
    harness.arm_memory_release(0)?;
    drop((first_completion, second_completion, first, second));
    assert_eq!(harness.fake.initializations.load(Ordering::Acquire), 0);
    assert!(lock(&harness.claims.root.value).pool.is_none());
    harness.wait_reserved(&original, 0)?;
    assert_eq!(original.snapshot().reserved_bytes, 0);
    harness.close(0)?;
    assert_eq!(original.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn late_cancelled_initialization_retains_actual_acquisition_and_original_scope() -> TestResult {
    let mut harness = Harness::new()?;
    let barrier = Arc::new(Barrier::default());
    *lock(&harness.fake.gate) = Some((Stage::Initialize, barrier.clone()));
    let spec = harness.spec(crate::CLAIM_POOL_BYTES)?;
    harness.claims.submit_initialize(
        &harness.client,
        &harness.scope,
        spec,
        PathBuf::from("E:\\fixture"),
    )?;
    barrier.wait(1)?;
    harness.scope.cancel();
    barrier.release();
    harness.drain()?;
    assert!(harness.claims.initialized);
    assert_eq!(harness.scope.snapshot().reserved_bytes, crate::CLAIM_POOL_BYTES);
    let mut acquired = false;
    let mut stopped = false;
    while let Some(event) = harness.claims.take_observation() {
        acquired |= matches!(event, ClaimObservation::CoordinatorClaimed(_));
        stopped |= matches!(event, ClaimObservation::Stopped { kind: TerminalKind::Cancelled, .. });
    }
    assert!(acquired && stopped);
    let original = harness.fresh_scope()?;
    assert_eq!(harness.claims.initialization_scope(), Some(original.id()));
    harness.close(0)?;
    harness.wait_reserved(&original, 0)?;
    assert_eq!(original.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn late_cancelled_backend_acquisition_is_retained_until_real_cleanup_job() -> TestResult {
    let mut harness = Harness::new()?;
    harness.initialize()?;
    let ticket = harness.resolve()?;
    let resource = harness.resource(1)?;
    let barrier = Arc::new(Barrier::default());
    *lock(&harness.fake.gate) = Some((Stage::Acquire, barrier.clone()));
    let spec = harness.spec(CLAIM_OPERATION_BYTES)?;
    harness.claims.submit_backend_claim(
        &harness.client,
        &harness.scope,
        spec,
        resource,
        &ticket,
    )?;
    barrier.wait(1)?;
    harness.scope.cancel();
    barrier.release();
    harness.drain()?;
    let entry = harness.claims.backends.iter().flatten().next().ok_or("Lost acquired backend")?;
    assert!(entry.claimed);
    assert!(!lock(&entry.shared.value).released);
    let original = harness.fresh_scope()?;
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
    harness.wait_reserved(&original, 0)?;
    assert_eq!(original.snapshot().reserved_bytes, 0);
    Ok(())
}
