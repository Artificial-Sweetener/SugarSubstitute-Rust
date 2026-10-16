// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Deterministic cleanup-cutoff and retained allocation policy regressions.
use super::*;
#[test]
fn first_recovery_cleanup_budget_survives_counter_failure_and_rejects_changed_transaction()
-> Result<()> {
    let first = ShutdownBudget::new(Moment::from_nanos(10))?;
    let mut closing = None;
    Closing::latch(&mut closing, first)?;
    assert!(closing.as_ref().is_some_and(|close| close.counter.is_none()));
    let changed = ShutdownBudget::new(Moment::from_nanos(11))?;
    assert!(Closing::latch(&mut closing, changed).is_err());
    Closing::latch(&mut closing, first)?;
    let counter = CounterDeadline::new(900, 100)?;
    closing.as_mut().ok_or("Missing original recovery cleanup")?.counter = Some(counter);
    Closing::latch(&mut closing, first)?;
    assert!(Closing::latch(&mut closing, changed).is_err());
    let original = closing.as_ref().ok_or("Original recovery cleanup lost")?;
    assert_eq!(original.budget, first);
    assert_eq!(original.counter, Some(counter));
    Ok(())
}
#[test]
fn recovery_metadata_keeps_existing_startup_and_worker_admission_partitions() {
    assert!(RecoveryStart::required_storage_bytes() <= 61_440);
    assert!(RecoveryStart::start_storage_bytes() > size_of::<State>() as u64);
    assert_eq!(
        RecoveryStart::required_storage_bytes(),
        RecoveryStart::start_storage_bytes() + RootRecoverySession::required_storage_bytes()
    );
    assert_eq!(RootRecoverySession::worker_reservation_bytes(), 4096);
}
