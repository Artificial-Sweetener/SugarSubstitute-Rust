// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise natural completion, exact Stop custody and actual write-receipt rejection at the owner.
use super::*;
#[test]
fn natural_child_exit_waits_for_root_stop_and_all_actual_local_retirement() -> Result<()> {
    let mut owner = RetirementNotice::new();
    let now = Moment::from_nanos(10);
    let until = Moment::from_nanos(100);
    assert_eq!(owner.action(now, until, true), Action::Wait);
    assert!(owner.queued(now, until, true).is_err());
    assert!(owner.written().is_err());
    assert!(owner.accept_stop(CounterDeadline::new(100, 10)?)?);
    assert_eq!(owner.action(now, until, false), Action::Wait);
    assert!(owner.queued(now, until, false).is_err());
    assert_eq!(owner.action(now, until, true), Action::Send);
    owner.queued(now, until, true)?;
    assert_eq!(owner.action(now, until, true), Action::Wait);
    assert!(owner.queued(now, until, true).is_err());
    owner.written()?;
    assert_eq!(owner.action(now, until, true), Action::Close);
    assert!(owner.written().is_err());
    Ok(())
}
#[test]
fn stop_replay_and_original_cutoff_cannot_renew_or_manufacture_retirement() -> Result<()> {
    let mut owner = RetirementNotice::new();
    let stop = CounterDeadline::new(100, 10)?;
    assert!(owner.accept_stop(stop)?);
    assert!(!owner.accept_stop(stop)?);
    assert!(owner.accept_stop(CounterDeadline::new(101, 10)?).is_err());
    assert_eq!(owner.action(Moment::from_nanos(100), Moment::from_nanos(100), true), Action::Close);
    assert_eq!(
        owner.action(Moment::from_nanos(101), Moment::from_nanos(100), false),
        Action::Close
    );
    let absent = RetirementNotice::new();
    assert_eq!(absent.action(Moment::from_nanos(99), Moment::from_nanos(100), true), Action::Wait);
    assert_eq!(
        absent.action(Moment::from_nanos(100), Moment::from_nanos(100), false),
        Action::Close
    );
    Ok(())
}

#[test]
fn original_failure_abandons_notice_without_advisory_retirement_or_clock_renewal() -> Result<()> {
    let mut owner = RetirementNotice::new();
    owner.accept_stop(CounterDeadline::new(100, 10)?)?;
    owner.abandon();
    let now = Moment::from_nanos(10);
    let until = Moment::from_nanos(100);
    assert!(owner.queued(now, until, true).is_err());
    assert!(owner.written().is_err());
    assert_eq!(owner.action(Moment::from_nanos(10), Moment::from_nanos(100), true), Action::Close);
    Ok(())
}
