// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The coordinator's SugarCubes schedule: what each preparation waits for, and in what order.
use super::*;
use crate::pack_install::{ReleaseSourceKind, ReleaseVersion, RestoreReason};
use sugarsubstitute_contracts::update::pack::PackOutcome;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn updated() -> Result<StartupOutcome, Box<dyn std::error::Error>> {
    Ok(StartupOutcome::Updated {
        from: ReleaseVersion::parse("1.0.0")?,
        to: ReleaseVersion::parse("1.1.0")?,
        source: ReleaseSourceKind::Registry,
        unchecked: Vec::new(),
    })
}

fn restored() -> Restored {
    Restored {
        version: "1.1.0".into(),
        restored_version: "1.0.0".into(),
        reason: RestoreReason::PageDidNotLoad,
    }
}

fn outcome(schedule: &PackSchedule<u8>) -> Option<PackOutcome> {
    match schedule.status().0 {
        PackUpdateStatus::Finished(report) => Some(report.outcome),
        _ => None,
    }
}

#[test]
fn the_first_managed_preparation_waits_for_the_update_and_shows_it() -> TestResult {
    let mut schedule = PackSchedule::default();
    assert!(schedule.prepare(1, true));
    assert!(schedule.gates(1));
    assert_eq!(schedule.status(), (&PackUpdateStatus::Updating, 1));
    assert_eq!(schedule.start_next(), Some((PackWork::Update { managed: true }, Some(1))));
    assert_eq!(schedule.start_next(), None, "one job at a time");
    assert!(schedule.gates(1));
    assert_eq!(schedule.ended(PackWorkEnded::Updated(updated()?)), Some(1));
    assert!(!schedule.gates(1));
    assert_eq!(outcome(&schedule), Some(PackOutcome::Updated));
    assert_eq!(schedule.status().1, 2);
    // Later preparations of an ordinary start wait for nothing.
    assert!(!schedule.prepare(2, true));
    assert_eq!(schedule.start_next(), None);
    Ok(())
}

#[test]
fn an_attached_comfyui_is_only_checked_and_never_waits_or_is_judged() -> TestResult {
    let mut schedule = PackSchedule::default();
    assert!(!schedule.prepare(1, false));
    assert!(!schedule.gates(1));
    assert_eq!(schedule.status().0, &PackUpdateStatus::Idle);
    assert_eq!(schedule.start_next(), Some((PackWork::Update { managed: false }, None)));
    assert_eq!(schedule.ended(PackWorkEnded::Updated(StartupOutcome::UpToDate)), None);
    schedule.observe(TrialEvent::BackendStartUnobservable);
    schedule.backend_stopped();
    schedule.page_failed();
    assert_eq!(schedule.start_next(), None);
    assert!(!schedule.prepare(2, false));
    Ok(())
}

#[test]
fn a_failed_page_holds_the_retry_behind_a_restore_of_the_previous_version() -> TestResult {
    let mut schedule = PackSchedule::default();
    schedule.prepare(1, true);
    schedule.start_next();
    schedule.ended(PackWorkEnded::Updated(updated()?));
    schedule.observe(TrialEvent::BackendStartUnobservable);
    schedule.page_failed();
    assert_eq!(
        schedule.start_next(),
        Some((PackWork::Observe(TrialEvent::BackendStartUnobservable), None))
    );
    // The retry arrives while the failed page is still being recorded: it waits for both.
    assert!(schedule.prepare(2, true));
    assert_eq!(schedule.status().0, &PackUpdateStatus::Updating);
    assert_eq!(schedule.ended(PackWorkEnded::Observed), None);
    assert!(schedule.gates(2));
    assert_eq!(schedule.start_next(), Some((PackWork::Observe(TrialEvent::PageFailed), None)));
    // Stopping the retry may cancel only work that holds it, never the page's record.
    assert!(!schedule.running_holds(2));
    assert_eq!(schedule.ended(PackWorkEnded::Observed), None);
    assert!(schedule.gates(2));
    assert_eq!(schedule.start_next(), Some((PackWork::RestoreFailedPage, Some(2))));
    assert_eq!(schedule.ended(PackWorkEnded::Restored(Some(restored()))), Some(2));
    assert_eq!(outcome(&schedule), Some(PackOutcome::Restored));
    // The restore happens once; a further retry waits for nothing.
    assert!(!schedule.prepare(3, true));
    Ok(())
}

#[test]
fn observations_run_in_order_and_an_unfinished_update_still_releases_the_start() -> TestResult {
    let mut schedule = PackSchedule::default();
    schedule.prepare(1, true);
    schedule.observe(TrialEvent::BackendStartUnobservable);
    schedule.backend_stopped();
    assert_eq!(schedule.start_next(), Some((PackWork::Update { managed: true }, Some(1))));
    assert_eq!(schedule.ended(PackWorkEnded::Unfinished { stopped: true }), Some(1));
    assert_eq!(outcome(&schedule), Some(PackOutcome::Stopped));
    assert_eq!(
        schedule.start_next(),
        Some((PackWork::Observe(TrialEvent::BackendStartUnobservable), None))
    );
    assert_eq!(schedule.ended(PackWorkEnded::Observed), None);
    assert_eq!(schedule.start_next(), Some((PackWork::BackendStopped, None)));
    assert_eq!(schedule.ended(PackWorkEnded::Restored(Some(restored()))), None);
    assert_eq!(outcome(&schedule), Some(PackOutcome::Restored));
    assert_eq!(schedule.start_next(), None);
    Ok(())
}
