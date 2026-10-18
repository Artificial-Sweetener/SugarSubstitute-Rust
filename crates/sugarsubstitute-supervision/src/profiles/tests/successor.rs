// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Prove successor ordering against the actual profile owner's external native boundary.
use super::*;
use sugarsubstitute_contracts::execution::{CompletionBatch, ScopeId};

/// Main retirement and foreign completion cannot discharge a native predecessor.
#[test]
fn same_profile_successor_waits_for_consumed_original_native_release() -> TestResult {
    let mut h = Harness::new()?;
    let path = PathBuf::from(r"E:\fixture\profile");
    assert!(h.lease.successor_retirement(&path).is_none());
    assert_eq!(h.acquire()?.kind, TerminalKind::Success);
    let barrier = h.lease.successor_retirement(&path).ok_or("Observed profile binding absent")?;
    assert_eq!(barrier.run(), run()?);
    assert!(!barrier.released());
    assert!(h.lease.successor_retirement(std::path::Path::new(r"E:\fixture\distinct")).is_none());
    assert!(
        h.lease.successor_retirement(std::path::Path::new(r"\\?\E:\fixture\profile")).is_none()
    );
    h.lease.retire_main_attempt(main(1)?)?;
    assert!(!barrier.released());
    let release = h.spec(100)?;
    h.lease.submit_release(&h.client, &h.scope, release, &h.registry)?;
    let batch = h.terminal_batch()?;
    assert!(!barrier.released());
    let mut foreign = CompletionBatch { notifications: batch.notifications.clone(), remaining: 0 };
    for notice in &mut foreign.notifications {
        notice.scope_id = ScopeId::new(99)?;
    }
    assert!(h.lease.consume_release(run()?, &foreign)?.is_none());
    assert!(!barrier.released());
    let mut foreign_run = run()?;
    foreign_run.instance = sugarsubstitute_contracts::lifecycle::InstanceId::new(2)?;
    assert!(h.lease.consume_release(foreign_run, &batch).is_err());
    assert!(!barrier.released());
    let observed = h.lease.consume_release(run()?, &batch)?.ok_or("Original release lost")?;
    assert_eq!((observed.job, observed.kind), (release.id, TerminalKind::Success));
    assert!(barrier.released());
    assert!(h.lease.successor_retirement(&path).is_none());
    h.finish_workers();
    Ok(())
}

/// A failed release keeps the original barrier closed through an exact-cutoff retry.
#[test]
fn failed_native_profile_release_cannot_open_its_successor_barrier() -> TestResult {
    let mut h = Harness::new()?;
    assert_eq!(h.acquire()?.kind, TerminalKind::Success);
    let barrier = h
        .lease
        .successor_retirement(std::path::Path::new(r"E:\fixture\profile"))
        .ok_or("Observed profile binding absent")?;
    h.fake.fail_release.store(true, Ordering::Release);
    let failed = h.release(100)?;
    assert_eq!(failed.kind, TerminalKind::Failure);
    assert!(failed.failure.is_some());
    assert!(!barrier.released());
    assert_eq!(
        h.lease.release_deadline,
        Some(sugarsubstitute_contracts::execution::Moment::from_nanos(100))
    );
    h.fake.fail_release.store(false, Ordering::Release);
    assert_eq!(h.release(100)?.kind, TerminalKind::Success);
    assert!(barrier.released());
    assert_eq!(h.fake.acquires.load(Ordering::Acquire), 1);
    h.finish_workers();
    Ok(())
}
