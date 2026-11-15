// SPDX-License-Identifier: AGPL-3.0-or-later
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
    assert_eq!(schedule.status(), (&PackUpdateStatus::Idle, 0), "not before it begins");
    assert_eq!(schedule.start_next(), Some((PackWork::Update { managed: true }, Some(1))));
    assert_eq!(schedule.status(), (&PackUpdateStatus::Updating, 1));
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
    let available = StartupOutcome::Available {
        version: ReleaseVersion::parse("1.1.0")?,
        unchecked: vec![ReleaseSourceKind::GitHubRelease],
    };
    assert_eq!(schedule.ended(PackWorkEnded::Updated(available)), None);
    assert_eq!(outcome(&schedule), Some(PackOutcome::Available), "the check is shown");
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
    assert_eq!(schedule.status().0, &PackUpdateStatus::Restoring);
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
    // The queued restore keeps the splash saying the previous version is being put back.
    assert_eq!(schedule.status().0, &PackUpdateStatus::Restoring);
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

#[test]
fn an_unattributed_page_failure_is_recorded_without_holding_the_retry() -> TestResult {
    let mut schedule = PackSchedule::default();
    schedule.prepare(1, true);
    schedule.start_next();
    schedule.ended(PackWorkEnded::Updated(updated()?));
    schedule.observe(TrialEvent::PageUnconfirmed);
    assert!(!schedule.prepare(2, true), "the same version may still load its page");
    assert_eq!(schedule.start_next(), Some((PackWork::Observe(TrialEvent::PageUnconfirmed), None)));
    assert_eq!(schedule.ended(PackWorkEnded::Observed), None);
    assert_eq!(outcome(&schedule), Some(PackOutcome::Updated));
    Ok(())
}

#[test]
fn an_attached_check_that_does_not_finish_says_so() -> TestResult {
    let mut schedule = PackSchedule::default();
    schedule.prepare(1, false);
    schedule.start_next();
    assert_eq!(schedule.ended(PackWorkEnded::Unfinished { stopped: true }), None);
    assert_eq!(outcome(&schedule), Some(PackOutcome::Stopped));
    Ok(())
}

#[test]
fn a_cancelled_startup_is_a_page_failure_of_the_judged_version() -> TestResult {
    let mut schedule = PackSchedule::default();
    schedule.prepare(1, true);
    schedule.start_next();
    schedule.ended(PackWorkEnded::Updated(updated()?));
    schedule.observe(TrialEvent::BackendStartUnobservable);
    schedule.startup_cancelled();
    assert_eq!(
        schedule.start_next(),
        Some((PackWork::Observe(TrialEvent::BackendStartUnobservable), None))
    );
    assert_eq!(schedule.ended(PackWorkEnded::Observed), None);
    assert_eq!(schedule.start_next(), Some((PackWork::Observe(TrialEvent::PageFailed), None)));
    assert_eq!(schedule.ended(PackWorkEnded::Observed), None);
    // A later managed start of the same coordinator restores the previous version first.
    assert!(schedule.prepare(2, true));
    assert_eq!(schedule.start_next(), Some((PackWork::RestoreFailedPage, Some(2))));
    Ok(())
}

#[test]
fn a_cancelled_startup_judges_nothing_in_an_attached_comfyui() {
    let mut schedule = PackSchedule::<u8>::default();
    schedule.prepare(1, false);
    schedule.start_next();
    schedule.ended(PackWorkEnded::Unfinished { stopped: true });
    schedule.startup_cancelled();
    assert_eq!(schedule.start_next(), None);
    assert!(!schedule.prepare(2, false));
}

#[test]
fn a_restore_is_given_an_installs_budget_and_only_observations_are_records() {
    assert_eq!(PackWork::Update { managed: true }.kind(), PackWorkKind::Download);
    assert_eq!(PackWork::Update { managed: false }.kind(), PackWorkKind::Download);
    assert_eq!(PackWork::BackendStopped.kind(), PackWorkKind::Replace);
    assert_eq!(PackWork::RestoreFailedPage.kind(), PackWorkKind::Replace);
    assert_eq!(PackWork::Observe(TrialEvent::PageFailed).kind(), PackWorkKind::Record);
}

/// A retry that arrives while a backend-failure restore runs waits for it, and stopping that
/// retry never cancels the restore; the splash says the previous version is being put back
/// until it is.
#[test]
fn a_retry_waits_for_a_running_restore_and_never_cancels_it() -> TestResult {
    let mut schedule = PackSchedule::default();
    schedule.prepare(1, true);
    schedule.start_next();
    schedule.ended(PackWorkEnded::Updated(updated()?));
    schedule.backend_stopped();
    assert_eq!(schedule.status().0, &PackUpdateStatus::Restoring);
    assert_eq!(schedule.start_next(), Some((PackWork::BackendStopped, None)));
    // The user's retry begins a preparation while the files are still being put back.
    assert!(schedule.prepare(2, true), "the retry must wait for the restore");
    assert!(schedule.gates(2));
    assert!(!schedule.running_holds(2), "stopping the retry must not cancel the restore");
    assert_eq!(schedule.ended(PackWorkEnded::Restored(Some(restored()))), Some(2));
    assert_eq!(outcome(&schedule), Some(PackOutcome::Restored));
    assert!(!schedule.gates(2));
    assert!(!schedule.prepare(3, true), "a later start waits for nothing");
    Ok(())
}

/// A retry that arrives before the queued restore even started also waits; a restore that
/// finds nothing to put back shows the update's report again.
#[test]
fn a_queued_restore_holds_the_retry_and_one_with_nothing_to_restore_shows_the_report_again()
-> TestResult {
    let mut schedule = PackSchedule::default();
    schedule.prepare(1, true);
    schedule.start_next();
    schedule.ended(PackWorkEnded::Updated(updated()?));
    schedule.observe(TrialEvent::BackendStartUnobservable);
    schedule.backend_stopped();
    assert!(schedule.prepare(2, true));
    assert_eq!(
        schedule.start_next(),
        Some((PackWork::Observe(TrialEvent::BackendStartUnobservable), None))
    );
    assert_eq!(schedule.ended(PackWorkEnded::Observed), None);
    assert_eq!(schedule.start_next(), Some((PackWork::BackendStopped, Some(2))));
    assert_eq!(schedule.ended(PackWorkEnded::Restored(None)), Some(2));
    assert_eq!(outcome(&schedule), Some(PackOutcome::Updated));
    Ok(())
}

/// While this launch may still restart into an application update, the launch-time update
/// waits; if the launch restarts, the preparation stopping leaves the update to the next copy.
/// An update already installing is reported as such, so a restart waits for it.
#[test]
fn the_launch_time_update_waits_while_an_application_restart_may_still_come() -> TestResult {
    let mut schedule = PackSchedule::default();
    schedule.hold_update(true);
    assert!(schedule.prepare(1, true));
    assert_eq!(schedule.start_next(), None, "held while the application check is pending");
    assert!(!schedule.installing());
    schedule.withdraw_update(1);
    assert!(!schedule.gates(1), "a stopping preparation no longer waits for it");
    assert_eq!(schedule.status().0, &PackUpdateStatus::Idle);
    assert_eq!(schedule.start_next(), None, "the restarted copy runs it");

    let mut schedule = PackSchedule::default();
    schedule.hold_update(true);
    assert!(schedule.prepare(1, true));
    schedule.hold_update(false);
    assert_eq!(schedule.start_next(), Some((PackWork::Update { managed: true }, Some(1))));
    assert!(schedule.installing(), "a restart now waits for the install");
    schedule.hold_update(true);
    assert!(schedule.installing(), "a running update is never held back");
    schedule.ended(PackWorkEnded::Updated(updated()?));
    assert!(!schedule.installing());
    assert_eq!(outcome(&schedule), Some(PackOutcome::Updated), "its real result is reported");
    Ok(())
}
