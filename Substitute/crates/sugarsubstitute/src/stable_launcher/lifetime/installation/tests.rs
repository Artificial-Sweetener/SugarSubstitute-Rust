// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise durable-choice routing, rejected admission and close with real executor custody.
use super::super::tests::{finish, fixture};
use super::*;

#[test]
fn absence_preserves_the_optional_cached_update_path() -> Result<()> {
    let (mut launcher, _, wake) = fixture()?;
    launcher.root = super::super::startup_tests::absent_installation();
    launcher.launch = None;
    launcher.installed(None);
    assert!(matches!(launcher.choice, Choice::Cache));
    launcher.select_next()?;
    assert!(matches!(launcher.job.as_ref().map(|pending| pending.task), Some(Task::Cache)));
    launcher.request_shutdown()?;
    finish(&mut launcher, &wake)
}

#[test]
fn durable_choice_enters_trial_transfer_before_any_creation() -> Result<()> {
    let (mut launcher, _, wake) = fixture()?;
    let launch = launcher.launch.take().ok_or("Fixture creator absent")?;
    launcher.installed(Some(launch));
    assert!(matches!(launcher.choice, Choice::Trial));
    assert!(launcher.cached.is_none());
    assert!(launcher.job.is_none());
    assert!(!launcher.completed);
    launcher.request_shutdown()?;
    finish(&mut launcher, &wake)
}

#[test]
fn observed_choice_after_close_retires_without_starting_a_trial() -> Result<()> {
    let (mut launcher, _, wake) = fixture()?;
    let launch = launcher.launch.take().ok_or("Fixture creator absent")?;
    launcher.request_shutdown()?;
    let cutoff = launcher.cleanup_until;
    launcher.installed(Some(launch));
    launcher.poll_shutdown()?;
    assert!(matches!(launcher.job.as_ref().map(|pending| pending.task), Some(Task::Retire)));
    finish(&mut launcher, &wake)?;
    assert!(!launcher.completed);
    assert_eq!(launcher.cleanup_until, cutoff);
    Ok(())
}

#[test]
fn damaged_installation_choice_preserves_failure_and_uses_only_accepted_fallback() -> Result<()> {
    let (mut launcher, _, wake) = fixture()?;
    launcher.launch = None;
    launcher.failed(Task::Installation, "installation authorization changed".into())?;
    assert!(matches!(launcher.choice, Choice::Accepted));
    assert!(launcher.cached.is_none());
    assert!(launcher.launch.is_none());
    assert!(launcher.recovery_until.is_none());
    assert_eq!(
        launcher.update_failure.as_ref().map(ToString::to_string).as_deref(),
        Some("installation authorization changed")
    );
    launcher.request_shutdown()?;
    finish(&mut launcher, &wake)
}

#[test]
fn rejected_choice_read_does_not_wait_for_a_nonexistent_completion() -> Result<()> {
    let (mut launcher, _, wake) = fixture()?;
    launcher.launch = None;
    launcher.work_scope = Some(
        launcher
            .client
            .as_ref()
            .ok_or("Client absent")?
            .create_native_scope(ScopeSpec { id: ScopeId::new(3)?, deadline: None }, None)?,
    );
    launcher.select_next().err().ok_or("Native-only scope admitted storage work")?;
    assert!(matches!(launcher.choice, Choice::Accepted));
    assert!(launcher.update_failure.is_some());
    assert!(launcher.job.is_none());
    assert_eq!(launcher.jobs.allocate()?.get(), 3);
    launcher.request_shutdown()?;
    finish(&mut launcher, &wake)
}
