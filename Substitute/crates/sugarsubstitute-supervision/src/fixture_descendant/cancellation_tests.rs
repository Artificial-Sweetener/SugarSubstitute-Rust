// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Queued cancellation uses the production blocking admission and owned closure port.
use super::tests::*;
use super::*;
use std::{num::NonZeroUsize, sync::atomic::Ordering};
use sugarsubstitute_contracts::execution::JobOutcome;
#[test]
fn queued_creation_cancellation_has_no_native_effect_and_can_retire_its_unused_owner() -> TestResult
{
    let rig = Rig::new()?;
    let first = Arc::new(Gate::default());
    let second = Arc::new(Gate::default());
    let release = Arc::new(Gate::default());
    let mut held = Vec::new();
    for (id, entered) in [(1, first.clone()), (2, second.clone())] {
        let release = release.clone();
        held.push(rig.client.submit_blocking(&rig.scope, specification(id, 4096)?, move |_| {
            entered.open();
            release.wait()
        })?);
    }
    first.wait()?;
    second.wait()?;
    let boundary = Boundary::new(false, false);
    let mut launch = FixtureDescendantLaunch::submit_with(
        &rig.client,
        &rig.scope,
        specification(3, 4096)?,
        input()?,
        Box::new(Fake(boundary.clone())),
    )?;
    launch.job.as_ref().ok_or("Queued creation missing")?.cancel();
    release.open();
    for job in &mut held {
        loop {
            let before = *lock(&rig.wake.count);
            let _ = rig.client.drain_completions(NonZeroUsize::new(32).ok_or("budget")?)?;
            if let Some(completion) = job.try_take_completion() {
                assert!(matches!(completion.outcome, JobOutcome::Success(())));
                break;
            }
            rig.wake.wait(before)?;
        }
    }
    assert!(matches!(rig.completion(&mut launch)?.outcome, JobOutcome::Cancelled));
    assert_eq!(boundary.creates.load(Ordering::Acquire), 0);
    assert_eq!(boundary.closes.load(Ordering::Acquire), 0);
    assert!(!launch.native_retired());
    launch.submit_retirement(&rig.client, &rig.scope, specification(4, 4096)?)?;
    assert!(matches!(rig.completion(&mut launch)?.outcome, JobOutcome::Success(())));
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    assert!(launch.native_retired());
    Ok(())
}
