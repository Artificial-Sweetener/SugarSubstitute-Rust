// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Actual optional-worker retirement and native creation-fact routing without launching application payloads.
use super::tests::{finish, fixture};
use super::*;

#[test]
fn native_entry_checks_installed_preparation_before_accepted_fallback() -> Result<()> {
    let wake = Arc::new(crate::updates::test_support::Wake::default());
    let mut launcher = Launcher::new(absent_installation());
    launcher.clock = Arc::new(crate::updates::test_support::Manual::default());
    launcher.start(wake.clone())?;
    let reads_installed_preparation =
        matches!(launcher.job.as_ref().map(|pending| pending.task), Some(Task::Installation));
    launcher.request_shutdown()?;
    if let Err(cause) = launcher.poll_shutdown() {
        assert_eq!(
            cause.downcast_ref::<io::Error>().map(io::Error::kind),
            Some(io::ErrorKind::NotFound),
            "{cause}"
        );
    }
    finish(&mut launcher, &wake)?;
    assert!(reads_installed_preparation);
    Ok(())
}

#[test]
fn negative_creator_reply_recovers_once_and_completes_without_another_offer() -> Result<()> {
    let (mut launcher, _, wake) = fixture()?;
    launcher.offer_observed(Some(CreationOutcome::NotCreated))?;
    assert!(matches!(launcher.choice, Choice::Recover));
    assert!(!launcher.completed);
    launcher.recovered(CreationOutcome::NotCreated)?;
    assert!(launcher.completed);
    assert!(matches!(launcher.choice, Choice::Complete));
    launcher.request_shutdown()?;
    finish(&mut launcher, &wake)
}

#[test]
fn actual_creation_survives_unsuccessful_engine_exit_without_accepted_fallback() -> Result<()> {
    let (mut launcher, _, wake) = fixture()?;
    launcher.activation_observed(ActivationOutcome::Stopped {
        exit_code: Some(9),
        creation: CreationOutcome::Created,
    })?;
    assert!(launcher.completed);
    assert!(matches!(launcher.choice, Choice::Complete));
    launcher.request_shutdown()?;
    finish(&mut launcher, &wake)
}

#[test]
fn uncertain_creator_reply_never_admits_source_recovery_or_accepted_fallback() -> Result<()> {
    let (mut launcher, _, wake) = fixture()?;
    launcher.choice = Choice::ObserveOffer;
    assert!(launcher.offer_observed(Some(CreationOutcome::Indeterminate)).is_err());
    assert!(!launcher.completed);
    assert!(matches!(launcher.choice, Choice::ObserveOffer));
    launcher.request_shutdown()?;
    finish(&mut launcher, &wake)
}
use crate::updates::test_support::observe;
use std::sync::atomic::{AtomicU64, Ordering};
use sugarsubstitute_supervision::update_preparation::CachedConfiguration;
static NEXT: AtomicU64 = AtomicU64::new(1);

/// Reserve a unique absent path without creating installation or repository state.
pub(super) fn absent_installation() -> PathBuf {
    std::env::temp_dir().join(format!(
        "missing-startup-cache-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::SeqCst)
    ))
}

/// Configure the real cache-only worker against a unique missing installation; it must never do HTTP.
pub(super) fn absent_cache() -> Result<cached::Cached> {
    let root = absent_installation();
    Ok(cached::Cached::new(Worker::resume_cached(CachedConfiguration {
        workspace: root.join("workspace"),
        installation: root,
    })?))
}

#[test]
fn missing_cached_trust_retires_before_accepted_selection_is_admitted() -> Result<()> {
    let (mut launcher, _, wake) = fixture()?;
    launcher.installed(None);
    launcher.cached = Some(absent_cache()?);
    observe(&wake, || {
        launcher.advance(NonZeroUsize::MIN.saturating_add(31))?;
        Ok(matches!(launcher.choice, Choice::Accepted))
    })?;
    assert!(launcher.cached.is_none());
    assert!(launcher.launch.is_none());
    assert!(matches!(launcher.job.as_ref().map(|pending| pending.task), Some(Task::Select)));
    launcher.request_shutdown()?;
    // The accepted selection uses the deliberately invalid fixture root. Its original failure
    // may already have completed before cancellation; cleanup must still finish afterwards.
    if let Err(cause) = launcher.poll_shutdown() {
        assert!(cause.downcast_ref::<io::Error>().is_some(), "{cause}");
    }
    finish(&mut launcher, &wake)
}

#[test]
fn closing_cache_before_admission_retires_it_without_selecting_or_creating() -> Result<()> {
    let (mut launcher, _, wake) = fixture()?;
    launcher.installed(None);
    launcher.cached = Some(absent_cache()?);
    launcher.request_shutdown()?;
    finish(&mut launcher, &wake)?;
    assert!(launcher.cached.is_none());
    assert!(!launcher.completed);
    assert!(matches!(launcher.choice, Choice::Accepted));
    Ok(())
}

#[test]
fn uncreated_trial_failure_observes_creator_recovery_before_fallback() -> Result<()> {
    let (mut launcher, _, wake) = fixture()?;
    launcher.failed(Task::Trial, "trial authority rejected".into())?;
    assert!(matches!(launcher.choice, Choice::Recover));
    assert!(launcher.launch.is_some());
    let cutoff = launcher.recovery_until;
    observe(&wake, || {
        launcher.advance(NonZeroUsize::MIN.saturating_add(31))?;
        Ok(matches!(launcher.choice, Choice::Accepted))
    })?;
    assert!(cutoff.is_some());
    assert!(launcher.recovery_until.is_none());
    assert!(launcher.launch.is_none());
    assert!(!launcher.completed);
    assert!(launcher.update_failure.is_some());
    launcher.request_shutdown()?;
    if let Err(cause) = launcher.poll_shutdown() {
        assert!(cause.downcast_ref::<io::Error>().is_some(), "{cause}");
    }
    finish(&mut launcher, &wake)
}

#[test]
fn engine_failure_requires_containment_settlement_before_accepted_fallback() -> Result<()> {
    let (mut launcher, _, wake) = fixture()?;
    launcher.launch = None;
    launcher.failed(Task::PrepareEngine, "engine capability rejected".into())?;
    assert!(matches!(launcher.choice, Choice::StopEngine));
    let cutoff = launcher.recovery_until;
    launcher.activation_observed(ActivationOutcome::Pending)?;
    assert!(matches!(launcher.choice, Choice::StopEngine));
    assert_eq!(launcher.recovery_until, cutoff);
    launcher.activation_observed(ActivationOutcome::Stopped {
        exit_code: None,
        creation: CreationOutcome::NotCreated,
    })?;
    assert!(matches!(launcher.choice, Choice::Accepted));
    assert!(launcher.launch.is_none());
    assert!(!launcher.completed);
    assert!(launcher.update_failure.is_some());
    launcher.request_shutdown()?;
    finish(&mut launcher, &wake)
}

#[test]
fn committed_engine_transfer_never_falls_back_after_discarded_observation() -> Result<()> {
    let (mut launcher, _, wake) = fixture()?;
    launcher.launch = None;
    launcher.failed(Task::ObserveEngine, "observation delivery was cancelled".into())?;
    launcher.activation_observed(ActivationOutcome::Stopped {
        exit_code: Some(0),
        creation: CreationOutcome::Created,
    })?;
    assert!(launcher.completed);
    assert!(matches!(launcher.choice, Choice::Complete));
    assert!(launcher.recovery_until.is_none());
    launcher.request_shutdown()?;
    finish(&mut launcher, &wake)
}

#[test]
fn engine_settlement_polls_keep_one_recovery_cutoff_and_close_never_launches_fallback() -> Result<()>
{
    let (mut launcher, clock, wake) = fixture()?;
    launcher.launch = None;
    launcher.choice = Choice::ObserveEngine;
    launcher.activation_observed(ActivationOutcome::Settling { exit_code: Some(7) })?;
    let cutoff = launcher.recovery_until;
    assert!(matches!(launcher.choice, Choice::StopEngine));
    assert!(!launcher.activation_due());
    clock.0.store(1_000_000_000, Ordering::SeqCst);
    assert!(launcher.activation_due());
    launcher.activation_failed("repeated activation observation failure".into())?;
    assert_eq!(launcher.recovery_until, cutoff);
    launcher.activation_observed(ActivationOutcome::Settling { exit_code: Some(7) })?;
    assert_eq!(launcher.recovery_until, cutoff);
    launcher.request_shutdown()?;
    assert_eq!(launcher.cleanup_until, cutoff);
    launcher.activation_observed(ActivationOutcome::Stopped {
        exit_code: Some(7),
        creation: CreationOutcome::NotCreated,
    })?;
    assert!(!matches!(launcher.choice, Choice::Accepted));
    assert!(!launcher.completed);
    finish(&mut launcher, &wake)
}

#[test]
fn receiving_engine_failure_never_selects_or_launches_an_application() -> Result<()> {
    let (mut launcher, _, wake) = fixture()?;
    launcher.launch = None;
    launcher.engine =
        Some(engine::Guard::new(launcher.root.clone(), std::env::current_exe()?, [92; 32]));
    launcher.choice = Choice::OpenEngine;
    launcher.select_next()?;
    observe(&wake, || match launcher.advance(NonZeroUsize::MIN.saturating_add(31)) {
        Ok(_) => Ok(false),
        Err(cause) => {
            assert!(cause.downcast_ref::<io::Error>().is_some(), "{cause}");
            Ok(true)
        }
    })?;
    assert!(launcher.launch.is_none());
    assert!(launcher.cached.is_none());
    assert!(!launcher.completed);
    launcher.request_shutdown()?;
    finish(&mut launcher, &wake)?;
    assert!(launcher.engine.is_none());
    Ok(())
}

#[test]
fn closing_receiving_engine_before_verification_retires_its_guard_on_cleanup_scope() -> Result<()> {
    let (mut launcher, _, wake) = fixture()?;
    launcher.launch = None;
    launcher.engine =
        Some(engine::Guard::new(launcher.root.clone(), std::env::current_exe()?, [92; 32]));
    launcher.choice = Choice::OpenEngine;
    launcher.request_shutdown()?;
    assert!(!launcher.poll_shutdown()?);
    assert!(matches!(launcher.job.as_ref().map(|job| job.task), Some(Task::Retire)));
    finish(&mut launcher, &wake)?;
    assert!(launcher.engine.is_none());
    assert!(!launcher.completed);
    Ok(())
}

#[test]
fn actual_creation_fact_prevents_relaunch_after_trial_error() -> Result<()> {
    let (mut launcher, _, wake) = fixture()?;
    launcher.failed(Task::Create { optional: true }, "creator handle cleanup failed".into())?;
    launcher.recovered(CreationOutcome::Created)?;
    assert!(launcher.completed);
    assert!(matches!(launcher.choice, Choice::Complete));
    assert!(launcher.launch.is_none());
    launcher.request_shutdown()?;
    finish(&mut launcher, &wake)
}

#[test]
fn uncertain_creation_is_reported_without_accepted_fallback_or_duplicate_launch() -> Result<()> {
    let (mut launcher, _, wake) = fixture()?;
    launcher.failed(Task::Create { optional: true }, "original uncertain creation".into())?;
    assert_eq!(
        launcher
            .recovered(CreationOutcome::Indeterminate)
            .err()
            .ok_or("Uncertain creation was accepted")?
            .to_string(),
        "original uncertain creation"
    );
    assert!(!launcher.completed);
    assert!(!matches!(launcher.choice, Choice::Accepted));
    launcher.request_shutdown()?;
    finish(&mut launcher, &wake)
}

#[test]
fn rejected_cached_job_admits_retirement_without_waiting_for_a_nonexistent_completion() -> Result<()>
{
    let (mut launcher, _, wake) = fixture()?;
    launcher.installed(None);
    launcher.cached = Some(absent_cache()?);
    launcher.work_scope = Some(
        launcher
            .client
            .as_ref()
            .ok_or("Client absent")?
            .create_native_scope(ScopeSpec { id: ScopeId::new(3)?, deadline: None }, None)?,
    );
    launcher.advance_cached()?;
    assert_eq!(
        launcher.jobs.allocate()?.get(),
        3,
        "Rejected work and accepted cleanup need distinct jobs in the same turn"
    );
    observe(&wake, || {
        launcher
            .client
            .as_ref()
            .ok_or("Client absent")?
            .drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        launcher.advance_cached()?;
        Ok(launcher.cached.is_none())
    })?;
    assert!(matches!(launcher.choice, Choice::Accepted));
    launcher.request_shutdown()?;
    finish(&mut launcher, &wake)
}

#[test]
fn rejected_trial_transfer_immediately_admits_creator_recovery() -> Result<()> {
    let (mut launcher, _, wake) = fixture()?;
    launcher.choice = Choice::Trial;
    launcher.work_scope = Some(
        launcher
            .client
            .as_ref()
            .ok_or("Client absent")?
            .create_native_scope(ScopeSpec { id: ScopeId::new(3)?, deadline: None }, None)?,
    );
    launcher.select_next()?;
    assert!(matches!(launcher.choice, Choice::Recover));
    let pending =
        launcher.job.as_ref().ok_or("Recovery was left waiting for a nonexistent wake")?;
    assert!(matches!(pending.task, Task::Recover));
    assert_eq!(pending.job.spec().deadline, launcher.recovery_until);
    launcher.request_shutdown()?;
    finish(&mut launcher, &wake)
}
