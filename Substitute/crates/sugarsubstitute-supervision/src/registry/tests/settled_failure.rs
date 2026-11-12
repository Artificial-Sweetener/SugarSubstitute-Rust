// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Transfer actual owner-produced failure only after central jobs and native-fake handles settle.
use super::*;
fn failed_family() -> Result<(Harness, ProcessKey), Box<dyn Error>> {
    let mut h = Harness::new()?;
    let key = h.launch(1)?.process();
    h.created(key)?;
    h.fake.root_alive.store(false, Ordering::Release);
    h.exit(key, ROOT, true, 0)?;
    h.stop(key)?;
    h.fake.fail_close.store(true, Ordering::Release);
    h.cleanup(key)?;
    assert!(!h.registry.snapshot(key)?.settled);
    assert!(matches!(h.registry.take_settled_failure(key), Err(RegistryAdmissionError::Busy)));
    h.fake.fail_close.store(false, Ordering::Release);
    h.clock.0.store(20_000_000_000, Ordering::Release);
    h.cleanup(key)?;
    assert!(h.registry.snapshot(key)?.settled);
    assert!(h.registry.snapshot(key)?.failed);
    Ok((h, key))
}
#[test]
fn settled_failure_move_keeps_exact_cause_vector_and_failed_record_until_explicit_retirement()
-> TestResult {
    let (mut h, key) = failed_family()?;
    let index = h.registry.index(key)?;
    let original = h.registry.records[index]
        .as_ref()
        .and_then(|record| record.error.as_ref())
        .ok_or("Actual original family failure missing")?;
    let cause = std::ptr::from_ref(original.source().ok_or("Actual source missing")?).cast::<()>();
    let cleanup = original.cleanup_causes().as_ptr();
    let stage = original.stage();
    let original_budget = h.registry.snapshot(key)?.shutdown;
    let moved = h.registry.take_settled_failure(key)?.ok_or("Original failure not moved")?;
    assert_eq!(moved.process(), key);
    assert_eq!(moved.cause().stage(), stage);
    assert_eq!(moved.cause().cleanup_causes().as_ptr(), cleanup);
    assert_eq!(
        std::ptr::from_ref(moved.cause().source().ok_or("Moved source missing")?).cast::<()>(),
        cause
    );
    assert!(h.registry.snapshot(key)?.failed);
    assert!(h.registry.snapshot(key)?.settled);
    assert_eq!(h.registry.snapshot(key)?.shutdown, original_budget);
    assert!(matches!(h.registry.take_settled_failure(key), Err(RegistryAdmissionError::Closed)));
    assert!(matches!(h.registry.retire_settled(key), Err(RegistryAdmissionError::Busy)));
    while let Some(observation) = h.registry.take_observation(key)? {
        assert!(!matches!(observation, RegistryObservation::Failed { .. }));
    }
    h.registry.retire_settled(key)?;
    assert!(h.registry.is_empty());
    assert_eq!(moved.cause().cleanup_causes().as_ptr(), cleanup);
    let original_bytes = h.table_scope.snapshot().reserved_bytes;
    assert!(original_bytes > 0);
    drop(h.tables);
    drop(h.registry);
    drop(h._executor);
    assert_eq!(h.table_scope.snapshot().reserved_bytes, original_bytes);
    drop(moved);
    assert_eq!(h.table_scope.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn settled_failure_transfer_rejects_foreign_and_running_family_without_moving_original()
-> TestResult {
    let mut h = Harness::new()?;
    let key = h.launch(1)?.process();
    h.created(key)?;
    assert!(matches!(h.registry.take_settled_failure(key), Err(RegistryAdmissionError::Busy)));
    let mut foreign = key;
    foreign.owner.epoch = CoordinatorEpoch::new([9; 16])?;
    assert!(matches!(
        h.registry.take_settled_failure(foreign),
        Err(RegistryAdmissionError::ForeignOwner)
    ));
    assert!(!h.registry.snapshot(key)?.settled);
    assert!(h.registry.snapshot(key)?.started);
    assert!(
        h.registry.records[h.registry.index(key)?]
            .as_ref()
            .is_some_and(|record| !record.failure_transferred)
    );
    h.fake.root_alive.store(false, Ordering::Release);
    h.exit(key, ROOT, true, 0)?;
    h.stop(key)?;
    h.cleanup(key)?;
    assert!(h.registry.snapshot(key)?.settled);
    Ok(())
}
