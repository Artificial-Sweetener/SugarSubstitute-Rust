// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify the production native lifetime policy with deterministic boundary facts.
use super::*;
use sugarsubstitute_contracts::lifecycle::{
    CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, UiChallengeId,
};
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn run() -> Result<RunKey, Box<dyn std::error::Error>> {
    Ok(RunKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        instance: InstanceId::new(3)?,
        generation: RunGeneration::new(4)?,
    })
}
fn challenge(run: RunKey, id: u64) -> Result<UiChallengeKey, Box<dyn std::error::Error>> {
    Ok(UiChallengeKey { run, id: UiChallengeId::new(id)? })
}
#[test]
fn actual_activity_order_uses_one_nonreused_revision_space() -> TestResult {
    let run = run()?;
    let mut owner = NativeActivity::new(run);
    for (index, fact) in [
        UiWatchdogActivity::Inactive,
        UiWatchdogActivity::VisibleActive,
        UiWatchdogActivity::Suspended,
        UiWatchdogActivity::Inactive,
    ]
    .into_iter()
    .enumerate()
    {
        let Some(DesktopStartupObservation::NativeActivity { run: actual, revision, activity }) =
            owner.observe(fact)?
        else {
            return Err("activity missing".into());
        };
        assert_eq!(actual, run);
        assert_eq!(revision.get(), index as u64 + 1);
        assert_eq!(activity, fact);
        assert!(owner.observe(fact)?.is_none());
    }
    owner.close();
    assert!(owner.observe(UiWatchdogActivity::VisibleActive)?.is_none());
    assert_eq!(owner.revision, 4);
    Ok(())
}
#[test]
fn exhausted_revision_preserves_prior_state_and_never_wraps() -> TestResult {
    let mut owner = NativeActivity::new(run()?);
    owner.observe(UiWatchdogActivity::Inactive)?;
    owner.revision = u64::MAX;
    assert!(owner.observe(UiWatchdogActivity::VisibleActive).is_err());
    assert_eq!(owner.revision, u64::MAX);
    assert_eq!(owner.published, Some(UiWatchdogActivity::Inactive));
    Ok(())
}
#[test]
fn challenge_requires_live_dispatch_and_all_current_run_fields() -> TestResult {
    let current = run()?;
    let mut owner = NativeActivity::new(current);
    let key = challenge(current, 1)?;
    let mut changed = [current; 4];
    changed[0].owner.namespace = NamespaceId::new([8; 32]);
    changed[1].owner.epoch = CoordinatorEpoch::new([8; 16])?;
    changed[2].instance = InstanceId::new(8)?;
    changed[3].generation = RunGeneration::new(8)?;
    for foreign in changed {
        assert!(owner.bind(foreign).is_err());
        assert!(owner.process_challenge(challenge(foreign, 1)?, true).is_err());
        assert_eq!(owner.last_challenge, 0);
    }
    assert!(owner.process_challenge(key, false).is_err());
    assert_eq!(owner.last_challenge, 0);
    assert!(
        matches!(owner.process_challenge(key, true)?, Some(DesktopStartupObservation::UiChallengeProcessed(actual)) if actual == key)
    );
    assert!(owner.process_challenge(key, true).is_err());
    assert!(owner.process_challenge(challenge(current, 2)?, true)?.is_some());
    owner.close();
    assert!(owner.process_challenge(challenge(current, 3)?, true)?.is_none());
    assert_eq!(owner.last_challenge, 2);
    Ok(())
}
#[test]
fn retirement_requires_all_actual_native_owners_but_precedes_execution_settlement() -> TestResult {
    let run = run()?;
    let mut owner = NativeActivity::new(run);
    let complete = RetirementFacts { main: true, splash: true, observers: true };
    assert!(owner.retirement(complete).is_none());
    owner.close();
    for bits in 0..7 {
        assert!(
            owner
                .retirement(RetirementFacts {
                    main: bits & 1 != 0,
                    splash: bits & 2 != 0,
                    observers: bits & 4 != 0
                })
                .is_none()
        );
        assert!(!owner.retired());
    }
    assert!(
        matches!(owner.retirement(complete), Some(DesktopStartupObservation::NativeRetired(actual)) if actual == run)
    );
    assert!(owner.retired());
    assert!(owner.retirement(complete).is_none());
    let custody = super::super::custody::LoopCustody::new();
    assert!(!custody.may_exit(super::super::custody::ShutdownFacts {
        closing: true,
        main_retired: true,
        splash_retired: true,
        execution_settled: false
    }));
    assert!(custody.may_exit(super::super::custody::ShutdownFacts {
        closing: true,
        main_retired: true,
        splash_retired: true,
        execution_settled: true
    }));
    Ok(())
}
