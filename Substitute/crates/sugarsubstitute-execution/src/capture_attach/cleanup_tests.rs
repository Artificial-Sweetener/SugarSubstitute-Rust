// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Preserve cleanup wake/terminal semantics while the actual execution owner stops.
use super::{test_support::*, *};
use crate::runtime_test_support::{Rig, TestResult};
use std::sync::atomic::Ordering;
use sugarsubstitute_contracts::execution::*;

/// Observe the actual cleanup timer after shutdown's concurrent first clock sample.
#[test]
fn stopping_keeps_original_cleanup_timers_after_acknowledged_success_until_actual_close()
-> TestResult {
    let mut rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Arc::new(Boundary::default());
    let _release_boundary = ReleaseBoundary::new(&rig.client.core, &boundary);
    boundary.ready.store(true, Ordering::Release);
    let mut job = submit(&rig, &scope, &boundary)?;
    let pin = take_pin(&rig, &mut job)?;
    let shared = pin.shared.clone();
    assert_eq!(scope.snapshot().terminal_unobserved, 0);
    assert_eq!(job.status().phase, JobPhase::Terminal(TerminalKind::Success));
    rig.owner.request_shutdown(Moment::from_nanos(100_000_000_000));
    assert!(lock(&rig.client.core.state).stopping);
    assert!(shared.custody.stopped());
    rig.clock.wait_until_registered(Moment::from_nanos(10_000_000_000))?;
    let CaptureCleanupSnapshot::Latched { budget, .. } = shared.custody.snapshot() else {
        return Err("Owner shutdown did not latch actual cleanup".into());
    };
    assert_eq!(budget.graceful_until, Moment::from_nanos(10_000_000_000));
    assert_eq!(budget.settle_until, Moment::from_nanos(15_000_000_000));
    let mut retirement = pin.release();
    boundary.retiring.wait()?;
    rig.clock.advance(10_000_000_000);
    rig.clock.wait_until_registered(budget.settle_until)?;
    assert!(retirement.try_take_report().is_none());
    rig.clock.advance(5_000_000_000);
    let failure = take_retirement_report(&rig, &mut retirement)?;
    assert!(matches!(failure, Err(types::CaptureAttachFailure::Uncertain)));
    assert!(retirement.try_take_report().is_none());
    assert!(!boundary.physical_closed.load(Ordering::Acquire));
    assert_eq!(scope.snapshot().reserved_bytes, 16_384);
    assert_eq!(job.status().phase, JobPhase::Terminal(TerminalKind::Success));
    assert!(job.try_take_completion().is_none());
    let CaptureCleanupSnapshot::Latched { budget: retained, .. } = shared.custody.snapshot() else {
        return Err("Cleanup facts changed after total expiry".into());
    };
    assert_eq!(retained, budget);
    boundary.settle()?;
    boundary.source_retired.wait()?;
    drop(job);
    drop(rig.owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    assert!(retirement.try_take_report().is_none());
    assert!(lock(&shared.custody.state).native_retired);
    assert!(lock(&shared.custody.state).source_retired);
    assert_eq!(boundary.retirements.load(Ordering::Acquire), 1);
    Ok(())
}
