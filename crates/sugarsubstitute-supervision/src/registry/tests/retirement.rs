// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retry partial retirement using immutable proofs and original charged ownership.
use super::*;

#[test]
fn partial_close_after_empty_membership_retries_remaining_handles_without_absent_job_query()
-> TestResult {
    let mut h = Harness::new()?;
    let key = h.launch(1)?.process();
    h.created(key)?;
    h.fake.root_alive.store(false, Ordering::Release);
    h.exit(key, ROOT, true, 0)?;
    h.stop(key)?;
    let budget = h.registry.snapshot(key)?.shutdown;
    h.fake.fail_close_after_job.store(true, Ordering::Release);
    h.cleanup(key)?;
    assert!(!h.registry.snapshot(key)?.settled);
    assert!(h.scope.snapshot().reserved_bytes >= crate::registry_input::FAMILY_STORAGE_BYTES);
    let index = h.registry.index(key)?;
    let shared = h.registry.records[index].as_ref().ok_or("Missing family")?.shared.clone();
    assert!(
        lock(&shared.native)
            .family
            .as_ref()
            .is_some_and(|f| f.job.is_none() && f.closure_committed)
    );
    h.fake.fail_inventory.store(true, Ordering::Release);
    h.clock.0.store(20_000_000_000, Ordering::Release);
    h.cleanup(key)?;
    assert!(h.registry.snapshot(key)?.settled);
    assert_eq!(h.registry.snapshot(key)?.shutdown, budget);
    assert_eq!(h.fake.closes.load(Ordering::Acquire), 1);
    assert_eq!(h.fake.forces.load(Ordering::Acquire), 0);
    Ok(())
}

#[test]
fn rejected_cleanup_admission_retires_unbound_watch_and_keeps_member_for_retry() -> TestResult {
    let mut h = Harness::new()?;
    h.fake.include_leaf.store(true, Ordering::Release);
    h.fake.leaf_alive.store(true, Ordering::Release);
    let key = h.launch(1)?.process();
    h.created(key)?;
    h.stop(key)?;
    h.cleanup(key)?;
    let index = h.registry.index(key)?;
    if let Some(record) = &mut h.registry.records[index] {
        record.error = None;
        record.unread[3] = false;
    }
    h.scope.cancel();
    let watch = h.spec(JobClass::NativeObservation, 4096)?;
    let step = h.spec(JobClass::InfrastructureLifecycle, 4096)?;
    assert!(matches!(
        h.registry.submit_cleanup_step(&h.client, key, &h.scope, step, Some(watch)),
        Err(RegistryAdmissionError::Execution(_))
    ));
    h.drain_until(|registry| registry.cleanup_watches == 0)?;
    assert!(h.registry.snapshot(key)?.member_watch_required);
    assert!(!h.registry.snapshot(key)?.failed);
    assert_eq!(h.registry.snapshot(key)?.retained_members, 2);
    assert!(!h.registry.snapshot(key)?.settled);
    Ok(())
}
