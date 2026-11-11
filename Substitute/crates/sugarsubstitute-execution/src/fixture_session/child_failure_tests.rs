// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain inherited child ownership through combined panic and cleanup failure.
use super::*;
use std::num::NonZeroUsize;

#[test]
fn child_panic_and_retirement_failure_hold_inherited_handles_until_late_close() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new(false, true);
    boundary.panic_start.store(true, Ordering::Release);
    boundary.retire_failures.store(usize::MAX, Ordering::Release);
    let mut job = permit(&rig, &scope)?.bind_lease(Box::new(Lease(boundary.clone())));
    boundary.retired.wait()?;
    let observed = rig.wake.count();
    boundary.close_release.open();
    rig.wake.wait_after(observed)?;
    assert!(job.try_take_completion().is_none());
    assert_eq!(scope.snapshot().reserved_bytes, MIN_FIXTURE_SESSION_BYTES);
    assert_eq!(boundary.ready_signals.load(Ordering::Acquire), 0);
    assert_eq!(boundary.closes.load(Ordering::Acquire), 0);
    boundary.retire_failures.store(0, Ordering::Release);
    rig.client.core.changed();
    let mut failures = 0;
    let completion = loop {
        let observed = rig.wake.count();
        if rig.client.drain_completions(NonZeroUsize::MIN).is_err() {
            failures += 1;
        }
        if let Some(completion) = job.try_take_completion() {
            break completion;
        }
        rig.wake.wait_after(observed)?;
    };
    assert!(failures != 0);
    let JobOutcome::Failure(error) = completion.outcome else {
        return Err("Child fatal/native cause disappeared".into());
    };
    assert_eq!(error.stage(), "unregister_fixture_challenge");
    assert!(error.contains_owner_panic());
    assert_eq!(
        error.owner_panic_cause().map(PanicCause::as_str),
        Some("fixture child preparation panic")
    );
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    Ok(())
}
