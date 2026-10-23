// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Final closure reuses retained inventory while other lifecycle owners hold their charges.
use super::*;
use sugarsubstitute_contracts::execution::JobOutcome;

#[test]
fn source_cleanup_admits_without_reserving_a_second_inventory() -> TestResult {
    let mut harness = Harness::new(false, false)?;
    harness.inspect()?;
    let lease = harness.owner.take_inspection()?.ok_or("Missing inspected source")?;
    let cleanup_bytes = 4096;
    let pressure = harness.executor.limits().memory_bytes - FRONTEND_SOURCE_BYTES - cleanup_bytes;
    let mut pressure_spec = Harness::spec(2, 2000)?;
    pressure_spec.reservation_bytes = pressure;
    let mut occupied =
        harness.client.submit_blocking(&harness.scope, pressure_spec, move |context| {
            context.memory().claim(pressure)
        })?;
    harness
        .wake
        .until(&harness.client, || matches!(occupied.status().phase, JobPhase::Terminal(_)))?;
    let held = occupied.try_take_completion().ok_or("Missing retained pressure")?;
    assert!(matches!(held.outcome, JobOutcome::Success(_)));
    let mut close_spec = Harness::spec(3, 2000)?;
    close_spec.reservation_bytes = cleanup_bytes;
    let mut close = harness.owner.submit_close(&harness.client, &harness.scope, close_spec)?;
    assert!(lease.validate_live().is_err());
    harness
        .wake
        .until(&harness.client, || matches!(close.status().phase, JobPhase::Terminal(_)))?;
    let terminal = close.try_take_completion().ok_or("Missing cleanup completion")?;
    assert!(matches!(terminal.outcome, JobOutcome::Success(())));
    assert!(crate::registry::lock(&harness.owner.shared.retained).is_none());
    assert_eq!(lease._claim.bytes(), FRONTEND_SOURCE_BYTES);
    assert_eq!(harness.boundary.releases.load(Ordering::Acquire), 2);
    drop(terminal);
    drop(close);
    drop(lease);
    drop(held);
    drop(occupied);
    harness.wake.until(&harness.client, || harness.scope.snapshot().reserved_bytes == 0)?;
    Ok(())
}

#[test]
fn inspection_and_cleanup_require_their_own_admission_allowances() -> TestResult {
    let mut harness = Harness::new(false, false)?;
    assert!(
        harness
            .owner
            .submit_inspection(&harness.client, &harness.scope, Harness::close_spec(1, 1000)?,)
            .is_err()
    );
    harness.inspect()?;
    let lease = harness.owner.take_inspection()?.ok_or("Missing inspected source")?;
    assert!(
        harness
            .owner
            .submit_close(&harness.client, &harness.scope, Harness::spec(2, 2000)?,)
            .is_err()
    );
    lease.validate_live()?;
    assert!(matches!(harness.close()?.outcome, JobOutcome::Success(())));
    assert!(lease.validate_live().is_err());
    Ok(())
}
