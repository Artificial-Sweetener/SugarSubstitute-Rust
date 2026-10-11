// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Pin parent checkpoints and supervised failure retirement through actual execution.
use super::*;

fn terminal(
    rig: &Rig,
    job: &mut OwnedJob<FixtureSessionReport, NativeObservationError>,
) -> TestResult<JobOutcome<FixtureSessionReport, NativeObservationError>> {
    let mut failures = 0;
    loop {
        let observed = rig.wake.count();
        if let Some(completion) = job.try_take_completion() {
            if rig.client.drain_completions(NonZeroUsize::MIN).is_err() {
                failures += 1;
            }
            assert!(failures != 0);
            return Ok(completion.outcome);
        }
        if rig.client.drain_completions(NonZeroUsize::MIN).is_err() {
            failures += 1;
        }
        if let Some(completion) = job.try_take_completion() {
            assert!(failures != 0);
            return Ok(completion.outcome);
        }
        rig.wake.wait_after(observed)?;
    }
}

#[test]
fn parent_panic_and_failed_retirement_retain_native_ownership_and_fatal_cause() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new(false, false);
    boundary.panic_start.store(true, Ordering::Release);
    boundary.hold_close.store(true, Ordering::Release);
    boundary.retirement_failures.store(usize::MAX, Ordering::Release);
    let mut session = session(&rig, &scope, &boundary)?;
    boundary.retired.wait()?;
    let observed = rig.wake.count();
    boundary.close_release.open();
    rig.wake.wait_after(observed)?;
    assert!(session.completion.try_take_completion().is_none());
    assert_eq!(scope.snapshot().reserved_bytes, MIN_FIXTURE_SESSION_BYTES);
    boundary.retirement_failures.store(0, Ordering::Release);
    rig.client.core.changed();
    let JobOutcome::Failure(error) = terminal(&rig, &mut session.completion)? else {
        return Err("Parent panic/retirement cause disappeared".into());
    };
    assert_eq!(error.stage(), "unregister_fixture_ready");
    assert!(error.contains_owner_panic());
    assert_eq!(
        error.owner_panic_cause().map(PanicCause::as_str),
        Some("fixture parent start panic")
    );
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    Ok(())
}

#[test]
fn parent_lease_destructor_is_supervised_before_terminal_delivery() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new(true, false);
    boundary.panic_drop.store(true, Ordering::Release);
    let mut session = session(&rig, &scope, &boundary)?;
    fact(&rig, &session.control, FixtureObservation::Ready)?;
    session.control.request_release()?;
    let JobOutcome::Panicked(cause) = terminal(&rig, &mut session.completion)? else {
        return Err("Parent lease destructor escaped supervision".into());
    };
    assert_eq!(cause.as_str(), "fixture parent destructor panic");
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    Ok(())
}

#[test]
fn challenge_crossing_deadline_preserves_fact_without_signalling_release() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new(true, false);
    boundary.hold_challenge.store(true, Ordering::Release);
    let mut session = session(&rig, &scope, &boundary)?;
    fact(&rig, &session.control, FixtureObservation::Ready)?;
    session.control.request_challenge()?;
    boundary.challenge_entered.wait()?;
    session.control.request_release()?;
    rig.clock.advance(1_000);
    boundary.challenge_continue.open();
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::TimedOut));
    assert_eq!(boundary.challenges.load(Ordering::Acquire), 1);
    assert_eq!(boundary.releases.load(Ordering::Acquire), 0);
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    Ok(())
}
#[test]
fn partial_registration_keeps_first_cause_until_real_cleanup() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new(false, true);
    boundary.hold_close.store(true, Ordering::Release);
    let mut session = session(&rig, &scope, &boundary)?;
    boundary.retired.wait()?;
    assert!(session.completion.try_take_completion().is_none());
    assert_eq!(session.control.take_child_events().err(), Some(FixtureSessionError::Closed));
    boundary.close_release.open();
    let JobOutcome::Failure(error) = rig.completion(&mut session.completion)?.outcome else {
        return Err("Missing setup cause".into());
    };
    assert_eq!(error.stage(), "partial_fixture_registration");
    Ok(())
}

#[test]
fn timeout_retires_without_release_and_cannot_reset_original_deadline() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new(false, false);
    let mut session = session(&rig, &scope, &boundary)?;
    fact(&rig, &session.control, FixtureObservation::Prepared)?;
    rig.clock.wait_until_registered(Moment::from_nanos(1_000))?;
    rig.clock.advance(1_000);
    assert!(matches!(rig.completion(&mut session.completion)?.outcome, JobOutcome::TimedOut));
    assert_eq!(boundary.releases.load(Ordering::Acquire), 0);
    Ok(())
}

#[test]
fn unsolicited_response_rejects_without_claiming_challenge() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new(false, false);
    let mut session = session(&rig, &scope, &boundary)?;
    fact(&rig, &session.control, FixtureObservation::Prepared)?;
    boundary.signal(true)?;
    let JobOutcome::Failure(error) = rig.completion(&mut session.completion)?.outcome else {
        return Err("Unsolicited response accepted".into());
    };
    assert_eq!(error.stage(), "fixture_unsolicited_response");
    assert_eq!(boundary.challenges.load(Ordering::Acquire), 0);
    Ok(())
}

#[test]
fn failed_challenge_retains_original_signal_failure_before_terminal() -> TestResult {
    let rig = Rig::new()?;
    let scope = scope(&rig)?;
    let boundary = Boundary::new(true, false);
    boundary.challenge_fail.store(true, Ordering::Release);
    let mut session = session(&rig, &scope, &boundary)?;
    fact(&rig, &session.control, FixtureObservation::Ready)?;
    session.control.request_challenge()?;
    let JobOutcome::Failure(error) = rig.completion(&mut session.completion)?.outcome else {
        return Err("Failed Challenge accepted".into());
    };
    assert_eq!(error.stage(), "signal_fixture_challenge");
    assert_eq!(boundary.releases.load(Ordering::Acquire), 0);
    Ok(())
}
