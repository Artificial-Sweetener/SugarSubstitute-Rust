// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! The launch stays inside its seconds budget on the injected clock: late update work gives way
//! to the accepted release, a busy record falls back to it, and the process always exits.
use super::startup_tests::absent_cache;
use super::tests::{finish, fixture};
use super::*;
use crate::stable_launcher::test_support::Hold;
use crate::updates::test_support::observe;
use std::sync::atomic::Ordering;
use sugarsubstitute_supervision::setup_storage::{RouteFailure, SelectionFailure};

/// Seconds on the manual clock.
fn seconds(value: u64) -> u64 {
    value * 1_000_000_000
}

#[test]
fn the_whole_launch_is_bounded_in_seconds_not_an_hour() -> Result<()> {
    let wake = Arc::new(crate::updates::test_support::Wake::default());
    let mut launcher = Launcher::new(super::startup_tests::absent_installation());
    launcher.clock = Arc::new(crate::updates::test_support::Manual::default());
    launcher.start(wake.clone())?;
    let budget = launcher.budget.ok_or("Launcher budget absent")?;
    assert_eq!(launcher.until, Some(budget.until()));
    assert_eq!(budget.until().checked_duration_since(budget.started())?, LauncherBudget::DURATION);
    assert!(LauncherBudget::DURATION.as_secs() <= 120);
    launcher.request_shutdown()?;
    if let Err(cause) = launcher.poll_shutdown() {
        assert!(cause.downcast_ref::<io::Error>().is_some(), "{cause}");
    }
    finish(&mut launcher, &wake)
}

#[test]
fn update_work_before_its_cutoff_is_admitted() -> Result<()> {
    let (mut launcher, clock, wake) = fixture()?;
    launcher.launch = None;
    clock.0.store(seconds(60) - 1, Ordering::SeqCst);
    launcher.installed(None);
    launcher.select_next()?;
    assert!(matches!(launcher.job.as_ref().map(|pending| pending.task), Some(Task::Cache)));
    assert!(launcher.update_failure.is_none());
    close(&mut launcher, &wake)
}

#[test]
fn update_work_after_its_cutoff_gives_way_to_the_accepted_release() -> Result<()> {
    let (mut launcher, clock, wake) = fixture()?;
    launcher.launch = None;
    clock.0.store(seconds(60), Ordering::SeqCst);
    launcher.installed(None);
    launcher.select_next()?;
    assert!(matches!(launcher.job.as_ref().map(|pending| pending.task), Some(Task::Select)));
    assert!(
        launcher.update_failure.as_ref().is_some_and(|cause| cause.is::<LateUpdate>()),
        "the skipped update is retained for its report"
    );
    close(&mut launcher, &wake)
}

#[test]
fn a_late_trial_recovers_its_creator_before_the_accepted_release() -> Result<()> {
    let (mut launcher, clock, wake) = fixture()?;
    clock.0.store(seconds(61), Ordering::SeqCst);
    launcher.choice = Choice::Create { optional: true };
    launcher.select_next()?;
    assert!(matches!(launcher.choice, Choice::Recover));
    assert!(matches!(launcher.job.as_ref().map(|pending| pending.task), Some(Task::Recover)));
    close(&mut launcher, &wake)
}

#[test]
fn cached_preparation_running_at_the_cutoff_retires_into_the_accepted_release() -> Result<()> {
    let (mut launcher, clock, wake) = fixture()?;
    launcher.launch = None;
    launcher.installed(None);
    launcher.cached = Some(absent_cache()?);
    clock.0.store(seconds(60), Ordering::SeqCst);
    launcher.stop_late_update()?;
    assert!(launcher.update_wake.is_none(), "the cutoff is observed once");
    assert!(
        launcher.cached.as_ref().and_then(cached::Cached::cleanup_until).is_some(),
        "the late preparation is retiring"
    );
    observe(&wake, || {
        launcher.advance(NonZeroUsize::MIN.saturating_add(31))?;
        Ok(launcher.cached.is_none())
    })?;
    assert!(matches!(launcher.job.as_ref().map(|pending| pending.task), Some(Task::Select)));
    close(&mut launcher, &wake)
}

#[test]
fn a_record_kept_busy_by_another_process_falls_back_to_the_accepted_release() -> Result<()> {
    let (mut launcher, _, wake) = fixture()?;
    let busy = io::Error::other(RouteFailure::Selection(SelectionFailure::LockTimeout));
    launcher.failed(Task::Route, Box::new(busy))?;
    assert!(matches!(launcher.choice, Choice::Accepted));
    assert!(launcher.accepted_only);
    assert!(launcher.update_failure.is_some());
    let foreign = io::Error::other(RouteFailure::Foreign);
    assert!(launcher.failed(Task::Route, Box::new(foreign)).is_err(), "other refusals report");
    launcher.request_shutdown()?;
    finish(&mut launcher, &wake)
}

/// Close while the next step may already have failed on the unopened fixture root, then
/// observe the ordinary retirement.
fn close(launcher: &mut Launcher, wake: &crate::updates::test_support::Wake) -> Result<()> {
    launcher.request_shutdown()?;
    observe(wake, || {
        loop {
            match launcher.poll_shutdown() {
                Ok(settled) => return Ok(settled),
                // The fixture root's own failure is reported once; polling then continues.
                Err(cause) if cause.downcast_ref::<io::Error>().is_some() => {}
                Err(cause) => return Err(cause),
            }
        }
    })?;
    finish(launcher, wake)
}

/// Admit a cleanup-scope job that blocks until released, ignoring cancellation, like a native
/// call that never returns.
fn stuck(launcher: &mut Launcher) -> Result<Arc<Hold>> {
    let hold = Hold::new();
    let held = hold.clone();
    launcher.submit(Task::Recover, move |_| {
        held.block();
        Ok(Work::Retired)
    })?;
    hold.entered();
    Ok(hold)
}

#[test]
fn a_worker_that_never_returns_cannot_keep_the_launcher_from_exiting() -> Result<()> {
    let (mut launcher, clock, _wake) = fixture()?;
    launcher.launch = None;
    launcher.recovery_until = Some(Moment::from_nanos(seconds(100)));
    let hold = stuck(&mut launcher)?;
    launcher.request_shutdown()?;
    assert!(!launcher.poll_shutdown()?);
    clock.0.store(seconds(16), Ordering::SeqCst);
    assert!(launcher.poll_shutdown().is_err(), "the cleanup cutoff is reported once");
    assert!(launcher.cutoff_reported);
    let wake_at = launcher.next_deadline();
    assert!(wake_at.is_some(), "the exit grace wakes the pump without a completion");
    clock.0.store(seconds(21) - 1, Ordering::SeqCst);
    assert!(!launcher.poll_shutdown()?);
    assert_eq!(launcher.next_deadline(), wake_at, "the exit moment never renews");
    clock.0.store(seconds(21), Ordering::SeqCst);
    assert!(launcher.poll_shutdown()?, "the pump can exit");
    assert!(launcher.abandoned);
    assert!(launcher.next_deadline().is_none());
    assert!(launcher.finish_shutdown().is_err(), "the unsettled cleanup is still reported");
    assert!(launcher.finished);
    assert!(launcher.owner.is_none());
    hold.release();
    Ok(())
}
