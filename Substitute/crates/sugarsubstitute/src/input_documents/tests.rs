// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Debounce policy on a deterministic clock, and store behavior on real execution and files.
use super::*;
use std::time::Duration;
use sugarsubstitute_contracts::execution::{Moment, TimeError};

#[cfg(windows)]
mod storage;

const POLICY: AutosavePolicy =
    AutosavePolicy { quiet: Duration::from_millis(500), longest: Duration::from_secs(2) };

fn at(milliseconds: u64) -> Moment {
    Moment::from_nanos(milliseconds * 1_000_000)
}

#[test]
fn bursts_of_changes_coalesce_into_one_save_after_a_quiet_period() -> Result<(), TimeError> {
    let mut schedule = AutosaveSchedule::new(POLICY, 4);
    assert!(!schedule.dirty());
    assert_eq!(schedule.deadline(), None);
    assert_eq!(schedule.begin(at(10_000)), None, "nothing changed");
    schedule.changed(4, at(0))?;
    assert!(!schedule.dirty(), "the saved generation is not a change");
    for (generation, time) in [(5, 0), (6, 100), (7, 200)] {
        schedule.changed(generation, at(time))?;
    }
    schedule.changed(6, at(300))?;
    assert_eq!(schedule.deadline(), Some(at(700)), "quiet period follows the latest change");
    assert_eq!(schedule.begin(at(699)), None);
    assert_eq!(schedule.begin(at(700)), Some(7));
    assert_eq!(schedule.deadline(), None, "one save runs at a time");
    schedule.changed(8, at(800))?;
    assert_eq!(schedule.begin(at(5_000)), None);
    schedule.finished(true, at(900))?;
    assert!(schedule.dirty());
    assert_eq!(schedule.deadline(), Some(at(1_300)));
    assert_eq!(schedule.begin(at(1_300)), Some(8));
    schedule.finished(true, at(1_400))?;
    assert!(!schedule.dirty());
    assert_eq!(schedule.deadline(), None);
    Ok(())
}

#[test]
fn continuous_changes_save_by_the_longest_wait_and_failures_retry() -> Result<(), TimeError> {
    let mut schedule = AutosaveSchedule::new(POLICY, 0);
    for step in 0..10 {
        schedule.changed(step + 1, at(step * 400))?;
    }
    assert_eq!(schedule.deadline(), Some(at(2_000)), "a burst never postpones beyond the cap");
    assert_eq!(schedule.begin(at(3_600)), Some(10));
    schedule.finished(false, at(4_000))?;
    assert!(schedule.dirty(), "a failed save leaves the content unsaved");
    assert_eq!(schedule.deadline(), Some(at(4_500)));
    assert_eq!(schedule.begin(at(4_500)), Some(10));
    schedule.finished(true, at(4_600))?;
    assert!(!schedule.dirty());
    schedule.finished(true, at(4_700))?;
    assert_eq!(schedule.deadline(), None, "finishing without a running save changes nothing");
    Ok(())
}
