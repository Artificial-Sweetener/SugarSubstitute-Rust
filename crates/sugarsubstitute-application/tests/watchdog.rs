// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the real per-run policy through its public native-fact boundary.
use std::error::Error;
use sugarsubstitute_application::supervision::{UiChallengePhase, UiWatchdog};
use sugarsubstitute_contracts::{execution::Moment, lifecycle::*};
type TestResult = Result<(), Box<dyn Error>>;
fn at(seconds: u64) -> Moment {
    Moment::from_nanos(seconds * 1_000_000_000)
}
fn run() -> Result<RunKey, IdentityError> {
    Ok(RunKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        instance: InstanceId::new(1)?,
        generation: RunGeneration::new(1)?,
    })
}
fn queued(run: RunKey) -> Result<(UiWatchdog, UiChallengeKey), Box<dyn Error>> {
    let mut watchdog = UiWatchdog::new(run, at(0));
    watchdog.set_activity(run, UiWatchdogActivity::VisibleActive, at(0))?;
    let Some(UiWatchdogEffect::Challenge { challenge }) = watchdog.poll(at(5))? else {
        return Err("expected native challenge".into());
    };
    Ok((watchdog, challenge))
}
fn dispatch(
    watchdog: &mut UiWatchdog,
    challenge: UiChallengeKey,
    sent: u64,
    received: u64,
) -> Result<(), LifecycleRejection> {
    watchdog.observe(
        challenge.run,
        UiWatchdogObservation::Dispatched { challenge, at: at(sent) },
        at(received),
    )
}

#[test]
fn inactive_and_exact_cadence_do_not_create_early_challenges() -> TestResult {
    let run = run()?;
    let mut watchdog = UiWatchdog::new(run, at(0));
    assert_eq!(watchdog.poll(at(100))?, None);
    watchdog.set_activity(run, UiWatchdogActivity::VisibleActive, at(100))?;
    assert_eq!(watchdog.poll(Moment::from_nanos(104_999_999_999))?, None);
    let Some(UiWatchdogEffect::Challenge { challenge }) = watchdog.poll(at(105))? else {
        return Err("expected exact cadence challenge".into());
    };
    assert_eq!(challenge.id.get(), 1);
    assert_eq!(watchdog.snapshot().phase, Some(UiChallengePhase::Queued));
    assert_eq!(watchdog.snapshot().queued_at, Some(at(105)));
    assert_eq!(watchdog.snapshot().next_challenge_at, None);
    Ok(())
}

#[test]
fn queued_effect_is_not_dispatch_and_cannot_create_suspicion_or_retry() -> TestResult {
    let run = run()?;
    let (mut watchdog, challenge) = queued(run)?;
    assert_eq!(watchdog.validate_dispatch(run, challenge), Ok(()));
    assert_eq!(
        watchdog.observe(run, UiWatchdogObservation::Responded { challenge }, at(5)),
        Err(LifecycleRejection::WrongPhase)
    );
    assert_eq!(watchdog.poll(at(1000))?, None);
    assert_eq!(watchdog.snapshot().pending, Some(challenge));
    assert_eq!(watchdog.snapshot().suspicion_at, None);
    assert!(!watchdog.snapshot().suspicious);
    Ok(())
}

#[test]
fn actual_dispatch_starts_exact_deadline_with_one_diagnostic_and_no_automatic_stop() -> TestResult {
    let run = run()?;
    let (mut watchdog, challenge) = queued(run)?;
    dispatch(&mut watchdog, challenge, 10, 10)?;
    assert_eq!(watchdog.validate_dispatch(run, challenge), Err(LifecycleRejection::WrongPhase));
    assert_eq!(watchdog.snapshot().suspicion_at, Some(at(40)));
    assert_eq!(watchdog.poll(Moment::from_nanos(39_999_999_999))?, None);
    assert_eq!(watchdog.poll(at(40))?, Some(UiWatchdogEffect::Unresponsive { challenge }));
    assert_eq!(watchdog.poll(at(1000))?, None);
    assert_eq!(watchdog.snapshot().pending, Some(challenge));
    assert!(watchdog.snapshot().suspicious);
    Ok(())
}

#[test]
fn delayed_dispatch_observation_cannot_extend_original_suspicion_interval() -> TestResult {
    let run = run()?;
    let (mut watchdog, challenge) = queued(run)?;
    dispatch(&mut watchdog, challenge, 5, 100)?;
    assert_eq!(watchdog.snapshot().suspicion_at, Some(at(35)));
    assert_eq!(watchdog.poll(at(100))?, Some(UiWatchdogEffect::Unresponsive { challenge }));
    assert_eq!(watchdog.poll(at(100))?, None);
    Ok(())
}

#[test]
fn matching_native_response_retires_key_and_starts_fresh_cadence() -> TestResult {
    let run = run()?;
    let (mut watchdog, challenge) = queued(run)?;
    dispatch(&mut watchdog, challenge, 5, 5)?;
    watchdog.observe(run, UiWatchdogObservation::Responded { challenge }, at(7))?;
    assert_eq!(watchdog.snapshot().pending, None);
    assert_eq!(watchdog.poll(at(11))?, None);
    let Some(UiWatchdogEffect::Challenge { challenge: next }) = watchdog.poll(at(12))? else {
        return Err("expected fresh challenge".into());
    };
    assert_eq!(next.id, challenge.id.next()?);
    let before = watchdog.snapshot();
    assert_eq!(
        watchdog.observe(run, UiWatchdogObservation::Responded { challenge }, at(12)),
        Err(LifecycleRejection::StaleIdentity)
    );
    assert_eq!(watchdog.snapshot(), before);
    Ok(())
}

#[test]
fn late_matching_ui_response_recovers_after_only_one_suspicion_effect() -> TestResult {
    let run = run()?;
    let (mut watchdog, challenge) = queued(run)?;
    dispatch(&mut watchdog, challenge, 5, 5)?;
    assert_eq!(watchdog.poll(at(35))?, Some(UiWatchdogEffect::Unresponsive { challenge }));
    watchdog.observe(run, UiWatchdogObservation::Responded { challenge }, at(40))?;
    assert!(!watchdog.snapshot().suspicious);
    assert_eq!(watchdog.snapshot().next_challenge_at, Some(at(45)));
    assert_eq!(watchdog.poll(at(44))?, None);
    assert!(matches!(watchdog.poll(at(45))?, Some(UiWatchdogEffect::Challenge { .. })));
    Ok(())
}

#[test]
fn native_inactivity_and_suspension_invalidate_queued_and_dispatched_effects() -> TestResult {
    let run = run()?;
    for activity in [UiWatchdogActivity::Inactive, UiWatchdogActivity::Suspended] {
        for was_dispatched in [false, true] {
            let (mut watchdog, challenge) = queued(run)?;
            if was_dispatched {
                dispatch(&mut watchdog, challenge, 5, 5)?;
            }
            watchdog.set_activity(run, activity, at(6))?;
            assert_eq!(
                watchdog.validate_dispatch(run, challenge),
                Err(LifecycleRejection::StaleIdentity)
            );
            assert_eq!(watchdog.poll(at(100))?, None);
            watchdog.set_activity(run, UiWatchdogActivity::VisibleActive, at(100))?;
            assert_eq!(watchdog.poll(at(104))?, None);
            let Some(UiWatchdogEffect::Challenge { challenge: next }) = watchdog.poll(at(105))?
            else {
                return Err("expected resumed challenge".into());
            };
            assert_ne!(next, challenge);
            let before = watchdog.snapshot();
            assert_eq!(
                dispatch(&mut watchdog, challenge, 105, 105),
                Err(LifecycleRejection::StaleIdentity)
            );
            assert_eq!(
                watchdog.observe(run, UiWatchdogObservation::Responded { challenge }, at(105)),
                Err(LifecycleRejection::StaleIdentity)
            );
            assert_eq!(watchdog.snapshot(), before);
        }
    }
    Ok(())
}

#[test]
fn identical_activity_and_duplicate_dispatch_do_not_reset_deadlines() -> TestResult {
    let run = run()?;
    let (mut watchdog, challenge) = queued(run)?;
    watchdog.set_activity(run, UiWatchdogActivity::VisibleActive, at(20))?;
    assert_eq!(watchdog.snapshot().queued_at, Some(at(5)));
    dispatch(&mut watchdog, challenge, 5, 20)?;
    let before = watchdog.snapshot();
    assert_eq!(dispatch(&mut watchdog, challenge, 20, 20), Err(LifecycleRejection::WrongPhase));
    watchdog.set_activity(run, UiWatchdogActivity::VisibleActive, at(30))?;
    assert_eq!(watchdog.snapshot(), before);
    assert_eq!(watchdog.poll(at(35))?, Some(UiWatchdogEffect::Unresponsive { challenge }));
    Ok(())
}

#[test]
fn separately_authenticated_run_and_claimed_generation_must_both_match() -> TestResult {
    let run = run()?;
    let (mut watchdog, challenge) = queued(run)?;
    let before = watchdog.snapshot();
    let fresh = RunKey { generation: run.generation.next()?, ..run };
    let foreign = RunKey {
        owner: CoordinatorKey { epoch: CoordinatorEpoch::new([3; 16])?, ..run.owner },
        ..run
    };
    for invalid in [fresh, foreign] {
        let expected = if invalid.owner != run.owner {
            LifecycleRejection::ForeignOwner
        } else {
            LifecycleRejection::StaleIdentity
        };
        assert_eq!(
            watchdog.set_activity(invalid, UiWatchdogActivity::Inactive, at(5)),
            Err(expected)
        );
        assert_eq!(watchdog.validate_dispatch(invalid, challenge), Err(expected));
        assert_eq!(
            watchdog.observe(
                invalid,
                UiWatchdogObservation::Dispatched { challenge, at: at(5) },
                at(5)
            ),
            Err(expected)
        );
        assert_eq!(
            watchdog.observe(
                run,
                UiWatchdogObservation::Dispatched {
                    challenge: UiChallengeKey { run: invalid, ..challenge },
                    at: at(5)
                },
                at(5)
            ),
            Err(expected)
        );
    }
    assert_eq!(watchdog.snapshot(), before);
    Ok(())
}

#[test]
fn invalid_clock_and_impossible_dispatch_time_reject_without_state_changes() -> TestResult {
    let run = run()?;
    let (mut watchdog, challenge) = queued(run)?;
    let before = watchdog.snapshot();
    assert_eq!(watchdog.poll(at(4)), Err(LifecycleRejection::InvalidClock));
    assert_eq!(
        watchdog.set_activity(run, UiWatchdogActivity::Inactive, at(4)),
        Err(LifecycleRejection::InvalidClock)
    );
    assert_eq!(dispatch(&mut watchdog, challenge, 4, 5), Err(LifecycleRejection::InvalidClock));
    assert_eq!(dispatch(&mut watchdog, challenge, 6, 5), Err(LifecycleRejection::InvalidClock));
    assert_eq!(watchdog.snapshot(), before);
    dispatch(&mut watchdog, challenge, 5, 5)?;
    assert_eq!(
        watchdog.observe(run, UiWatchdogObservation::Responded { challenge }, at(4)),
        Err(LifecycleRejection::InvalidClock)
    );
    Ok(())
}

#[test]
fn overflowing_cadence_dispatch_and_response_deadlines_reject_atomically() -> TestResult {
    let run = run()?;
    let max = Moment::from_nanos(u64::MAX);
    let mut inactive = UiWatchdog::new(run, max);
    let before = inactive.snapshot();
    assert_eq!(
        inactive.set_activity(run, UiWatchdogActivity::VisibleActive, max),
        Err(LifecycleRejection::InvalidClock)
    );
    assert_eq!(inactive.snapshot(), before);
    let (mut watchdog, challenge) = queued(run)?;
    let before = watchdog.snapshot();
    assert_eq!(
        watchdog.observe(run, UiWatchdogObservation::Dispatched { challenge, at: max }, max),
        Err(LifecycleRejection::InvalidClock)
    );
    assert_eq!(watchdog.snapshot(), before);
    dispatch(&mut watchdog, challenge, 5, 5)?;
    let before = watchdog.snapshot();
    assert_eq!(
        watchdog.observe(run, UiWatchdogObservation::Responded { challenge }, max),
        Err(LifecycleRejection::InvalidClock)
    );
    assert_eq!(watchdog.snapshot(), before);
    assert_eq!(watchdog.poll(at(35))?, Some(UiWatchdogEffect::Unresponsive { challenge }));
    Ok(())
}
