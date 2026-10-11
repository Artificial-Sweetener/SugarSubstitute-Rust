// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bound each admitted force attempt and preserve failures before force.
use super::*;

#[test]
fn actual_clock_is_resampled_after_finite_native_inventory_crosses_grace() -> TestResult {
    let mut h = Harness::new()?;
    let key = h.launch(1)?.process();
    h.created(key)?;
    h.stop(key)?;
    h.clock.0.store(9_999_999_999, Ordering::Release);
    *lock(&h.fake.advance_clock) = Some(h.clock.clone());
    h.cleanup(key)?;
    assert_eq!(h.fake.forces.load(Ordering::Acquire), 1);
    assert!(!h.registry.snapshot(key)?.settled);
    Ok(())
}

#[test]
fn failed_force_is_not_repeated_in_the_same_admitted_step() -> TestResult {
    for prior_failure in [true, false] {
        let mut h = Harness::new()?;
        let key = h.launch(1)?.process();
        h.created(key)?;
        h.stop(key)?;
        h.fake.fail_terminate.store(true, Ordering::Release);
        h.fake.fail_inventory.store(prior_failure, Ordering::Release);
        h.clock.0.store(10_000_000_000, Ordering::Release);
        let index = h.registry.index(key)?;
        if let Some(record) = &mut h.registry.records[index] {
            record.error = None;
            record.unread[3] = false;
        }
        h.cleanup(key)?;
        assert_eq!(h.fake.forces.load(Ordering::Acquire), 1);
        let cause = h.registry.records[index]
            .as_ref()
            .and_then(|r| r.error.as_ref())
            .ok_or("Missing force failure")?;
        assert_eq!(
            cause.stage(),
            if prior_failure { "complete_job_inventory" } else { "terminate_job" }
        );
        assert_eq!(cause.cleanup_causes().len(), usize::from(prior_failure));
        h.cleanup(key)?;
        assert_eq!(h.fake.forces.load(Ordering::Acquire), 2);
        assert!(!h.registry.snapshot(key)?.settled);
    }
    Ok(())
}
