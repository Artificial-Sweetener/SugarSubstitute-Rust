// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The launcher's budget and notice decide from injected moments only.
use super::*;

fn at(seconds: u64, millis: u64) -> Moment {
    Moment::from_nanos(seconds * 1_000_000_000 + millis * 1_000_000)
}

#[test]
fn every_launcher_moment_is_in_seconds_from_its_own_start() -> Result<(), TimeError> {
    let budget = LauncherBudget::new(at(7, 0))?;
    assert_eq!(budget.started(), at(7, 0));
    assert_eq!(budget.notice_at(), at(9, 0));
    assert_eq!(budget.update_until(), at(67, 0));
    assert_eq!(budget.until(), at(127, 0));
    assert_eq!(LauncherBudget::exit_at(at(140, 0))?, at(145, 0));
    assert!(LauncherBudget::new(Moment::from_nanos(u64::MAX)).is_err());
    Ok(())
}

#[test]
fn optional_update_steps_stop_being_admitted_at_their_cutoff() -> Result<(), TimeError> {
    let budget = LauncherBudget::new(at(0, 0))?;
    assert!(budget.admits_update(at(0, 0)));
    assert!(budget.admits_update(at(59, 999)));
    assert!(!budget.admits_update(at(60, 0)));
    assert!(!budget.admits_update(at(119, 0)));
    Ok(())
}

#[test]
fn a_fast_launch_never_shows_the_notice() -> Result<(), TimeError> {
    let mut notice = StartingNotice::new(LauncherBudget::new(at(0, 0))?);
    assert_eq!(notice.next_wake(), Some(at(2, 0)));
    assert_eq!(notice.observe(at(0, 400), true), None);
    assert_eq!(notice.observe(at(1, 999), true), None);
    assert_eq!(notice.observe(at(1, 999), false), None);
    assert_eq!(notice.phase(), NoticePhase::Withdrawn);
    assert_eq!(notice.next_wake(), None);
    assert_eq!(notice.observe(at(30, 0), true), None, "a handed-off launch never shows it");
    Ok(())
}

#[test]
fn a_slow_launch_shows_the_notice_after_two_seconds_until_it_hands_off() -> Result<(), TimeError> {
    let mut notice = StartingNotice::new(LauncherBudget::new(at(5, 0))?);
    assert_eq!(notice.observe(at(6, 999), true), None);
    assert_eq!(notice.phase(), NoticePhase::Waiting);
    assert_eq!(notice.observe(at(7, 0), true), Some(NoticeChange::Show));
    assert_eq!(notice.next_wake(), None);
    assert_eq!(notice.observe(at(20, 0), true), None, "shown once, not again");
    assert_eq!(notice.observe(at(21, 0), false), Some(NoticeChange::Withdraw));
    assert_eq!(notice.observe(at(22, 0), true), None, "withdrawn for good");
    assert_eq!(notice.phase(), NoticePhase::Withdrawn);
    Ok(())
}
