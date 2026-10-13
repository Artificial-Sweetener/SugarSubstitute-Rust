// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep transferred launch ownership independent of the completed job handle.
use super::{test_support::*, *};
use crate::runtime_test_support::{Rig, TestResult};
use std::{num::NonZeroUsize, sync::atomic::Ordering};
use sugarsubstitute_contracts::execution::*;

/// Job cancellation/drop cannot revoke a taken pin; real scope stop and untaken drop can.
#[test]
fn acknowledged_pin_survives_job_cancel_and_drop_while_untaken_pin_still_cleans_up() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Arc::new(Boundary::default());
    let _release_boundary = ReleaseBoundary::new(&rig.client.core, &boundary);
    boundary.ready.store(true, Ordering::Release);
    let mut job = submit(&rig, &scope, &boundary)?;
    let pin = take_pin(&rig, &mut job)?;
    job.cancel();
    assert!(matches!(pin.shared.custody.snapshot(), CaptureCleanupSnapshot::Dormant));
    drop(job);
    {
        let state = lock(&rig.client.core.state);
        let record = state.jobs.get(&JobId::new(1)?).ok_or("Transferred pin record lost")?;
        assert!(record.acknowledged && record.terminal_published && record.held_native());
        assert_eq!(record.phase, JobPhase::Terminal(TerminalKind::Success));
        assert!(!record.retiring && !record.job.cancellation().is_requested());
        assert!(record.memory.is_some());
        assert!(
            record
                .native_custody
                .as_ref()
                .is_some_and(|actual| { Arc::ptr_eq(actual, &pin.shared.custody) })
        );
    }
    assert!(matches!(pin.shared.custody.snapshot(), CaptureCleanupSnapshot::Dormant));
    {
        let state = lock(&pin.shared.state);
        assert!(state.pinned && state.handles == Some([41, 42]) && state.endpoint.is_some());
    }
    assert_eq!(scope.snapshot().reserved_bytes, 16_384);
    assert_eq!(scope.snapshot().terminal_unobserved, 0);
    scope.cancel();
    assert!(pin.shared.custody.stopped());
    rig.clock.wait_until_registered(Moment::from_nanos(10_000_000_000))?;
    let CaptureCleanupSnapshot::Latched { budget, .. } = pin.shared.custody.snapshot() else {
        return Err("Original scope cleanup did not latch".into());
    };
    assert_eq!(budget.graceful_until, Moment::from_nanos(10_000_000_000));
    assert_eq!(budget.settle_until, Moment::from_nanos(15_000_000_000));
    let mut retirement = pin.release();
    boundary.retiring.wait()?;
    assert!(retirement.try_take_report().is_none());
    boundary.settle()?;
    boundary.source_retired.wait()?;
    take_retirement_report(&rig, &mut retirement)??;
    drop(rig.owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);

    let rig = Rig::new()?;
    let scope = test_support::scope(&rig)?;
    let boundary = Arc::new(Boundary::default());
    let _release_boundary = ReleaseBoundary::new(&rig.client.core, &boundary);
    boundary.ready.store(true, Ordering::Release);
    let job = submit(&rig, &scope, &boundary)?;
    loop {
        let previous = rig.wake.count();
        rig.client.drain_completions(NonZeroUsize::MIN)?;
        if job.status().phase == JobPhase::Terminal(TerminalKind::Success) {
            break;
        }
        rig.wake.wait_after(previous)?;
    }
    let custody = {
        let state = lock(&rig.client.core.state);
        let record = state.jobs.get(&JobId::new(1)?).ok_or("Untaken pin record lost")?;
        assert!(!record.acknowledged);
        record.native_custody.clone().ok_or("Untaken native custody lost")?
    };
    drop(job);
    assert!(custody.stopped());
    boundary.retiring.wait()?;
    assert!(!boundary.physical_closed.load(Ordering::Acquire));
    assert_eq!(scope.snapshot().reserved_bytes, 16_384);
    boundary.settle()?;
    boundary.source_retired.wait()?;
    drop(rig.owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
