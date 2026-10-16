// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Test actual local owner admission and native absence using deterministic execution time.
use super::*;
use crate::production::child::requests::RequestError;
use crate::production::child::test_support::child;
use std::sync::atomic::Ordering;
#[test]
fn local_acquisition_retirement_requires_actual_current_absence_and_never_synthesizes_remote_ack()
-> Result<()> {
    let (mut c, _owner, _clock) = child()?;
    c.begin_attempt_cleanup()?;
    assert!(!c.attempt.as_ref().ok_or("Cleanup absent")?.complete());
    assert!(c.take_startup_event()?.is_none());
    assert!(
        matches!(c.take_command()?,Some(DesktopStartupCommand::ConfirmMainAbsent(id)) if id==c.identity)
    );
    assert!(
        c.record_local_main_absence(
            MainIdentity { navigation: Generation::new(2)?, ..c.identity },
            false
        )
        .is_err()
    );
    assert!(c.record_local_main_absence(c.identity, true).is_err());
    assert!(!c.attempt.as_ref().ok_or("Cleanup lost")?.complete());
    c.observe(DesktopStartupObservation::MainAbsent(c.identity), None)?;
    let actual = c.take_startup_event()?.ok_or("Real local settlement absent")?;
    assert_eq!(actual.launch(), c.identity.launch);
    assert!(matches!(actual.observation(), StartupObservation::ConnectionSettled));
    let a = c.attempt.as_ref().ok_or("Cleanup lost")?;
    assert!(a.complete());
    assert!(a.remote_state() == (false, false, false));
    assert!(!c.shutdown.main_retired && !c.shutdown.main_retirement_ack);
    c.advance_attempt()?;
    assert!(c.take_startup_event()?.is_none());
    assert!(c.take_command()?.is_none());
    assert!(c.observe(DesktopStartupObservation::MainAbsent(c.identity), None).is_err());
    Ok(())
}
#[test]
fn local_acquisition_retirement_keeps_original_cutoff_and_cleanup_failure_blocks_retry()
-> Result<()> {
    let (mut c, _owner, clock) = child()?;
    c.begin_attempt_cleanup()?;
    let original = c.attempt.as_ref().ok_or("Cleanup absent")?.original_deadline();
    clock.0.store(original.as_nanos(), Ordering::SeqCst);
    assert!(c.observe(DesktopStartupObservation::MainAbsent(c.identity), None).is_err());
    assert!(!c.attempt.as_ref().ok_or("Cleanup lost")?.complete());
    assert_eq!(c.attempt.as_ref().ok_or("Cleanup lost")?.original_deadline(), original);
    c.retain_attempt_failure(Box::new(std::io::Error::other("genuine cleanup failure")))?;
    assert!(
        c.ensure_retry_ready(MainIdentity { launch: LaunchId::new(2)?, ..c.identity }).is_err()
    );
    assert!(c.cleanup_failed());
    Ok(())
}
#[test]
fn local_acquisition_retirement_event_capacity_retains_absence_without_fake_settlement()
-> Result<()> {
    let (mut c, _owner, _clock) = child()?;
    c.begin_attempt_cleanup()?;
    for slot in &mut c.events {
        *slot = Some(StartupEvent::new(c.identity.launch, StartupObservation::SplashPresented));
    }
    assert!(c.observe(DesktopStartupObservation::MainAbsent(c.identity), None).is_err());
    let a = c.attempt.as_ref().ok_or("Cleanup lost")?;
    assert!(!a.complete() && a.remote_state() == (false, false, false));
    assert!(
        c.ensure_retry_ready(MainIdentity { launch: LaunchId::new(2)?, ..c.identity }).is_err()
    );
    Ok(())
}

fn queued(c: &ProductionChild, original: Moment) -> Result<PendingAcquire> {
    Ok(PendingAcquire {
        kind: sugarsubstitute_application::startup::ConnectionBudgetKind::Attached,
        header: WireHeader {
            run: c.run,
            connection: Some(ConnectionId::new(1)?),
            request: Some(RequestId::new(2)?),
            deadline_qpc: 100,
        },
        original,
        admitted: false,
        retiring: false,
    })
}
#[test]
fn acquisition_timeout_routes_actual_original_cutoff_once_and_preserves_error() -> Result<()> {
    let (mut c, _owner, clock) = child()?;
    let original = Moment::from_nanos(100);
    c.acquire = Some(queued(&c, original)?);
    c.acquisition_header = c.acquire.map(|p| p.header);
    clock.0.store(99, Ordering::SeqCst);
    assert!(!c.expire_acquisition()?);
    clock.0.store(100, Ordering::SeqCst);
    assert!(c.expire_acquisition()?);
    let event = c.take_startup_event()?.ok_or("Real original timeout absent")?;
    let StartupObservation::Failed(failure) = event.into_observation() else {
        return Err("Wrong actual event".into());
    };
    assert_eq!(failure.code(), StartupFailureCode::TimedOut);
    assert_eq!(
        failure.source().and_then(|e| e.downcast_ref::<std::io::Error>()).map(std::io::Error::kind),
        Some(std::io::ErrorKind::TimedOut)
    );
    assert_eq!(c.acquire.ok_or("Original acquisition lost")?.original, original);
    assert!(c.acquire.ok_or("Original acquisition lost")?.retiring);
    assert!(c.expire_acquisition()?);
    assert!(c.take_startup_event()?.is_none());
    assert!(c.attempt.is_none() && c.frontend_budget.is_some());
    Ok(())
}
#[test]
fn acquisition_timeout_capacity_preserves_original_cause_and_fences_retry_without_disposal()
-> Result<()> {
    let (mut c, _owner, clock) = child()?;
    let original = Moment::from_nanos(100);
    let pending = queued(&c, original)?;
    c.acquire = Some(pending);
    for slot in &mut c.events {
        *slot = Some(StartupEvent::new(c.identity.launch, StartupObservation::SplashPresented));
    }
    clock.0.store(100, Ordering::SeqCst);
    assert!(c.expire_acquisition().is_err());
    assert_eq!(
        c.failure
            .as_ref()
            .and_then(|e| e.downcast_ref::<std::io::Error>())
            .map(std::io::Error::kind),
        Some(std::io::ErrorKind::TimedOut)
    );
    let retained = c.acquire.ok_or("Original acquisition lost")?;
    assert_eq!(retained.header, pending.header);
    assert_eq!(retained.original, original);
    assert!(!retained.retiring);
    assert!(
        c.ensure_retry_ready(MainIdentity { launch: LaunchId::new(2)?, ..c.identity }).is_err()
    );
    Ok(())
}

#[test]
fn acquisition_correlated_boundary_expiry_routes_only_exact_current_owner_and_preserves_cause()
-> Result<()> {
    let (mut c, _owner, _clock) = child()?;
    let mut ledger = requests::tests::requests_for_run(c.run)?;
    let original = Moment::from_nanos(100);
    let h = ledger.prepare(original, CounterDeadline::new(100, 100)?)?;
    ledger.acquisition(h, sugarsubstitute_application::startup::ConnectionBudgetKind::Attached)?;
    let pending = PendingAcquire {
        kind: sugarsubstitute_application::startup::ConnectionBudgetKind::Attached,
        header: h,
        original,
        admitted: false,
        retiring: false,
    };
    c.acquire = Some(pending);
    c.acquisition_header = Some(h);
    let rejected = ledger
        .outbound(h, original, original)
        .err()
        .ok_or("Real original cutoff rejection absent")?;
    let cause = ledger.expiry(h, rejected)?;
    c.recover_acquisition_expiry(Box::new(cause))?;
    let event = c.take_startup_event()?.ok_or("Original boundary failure absent")?;
    let StartupObservation::Failed(failure) = event.into_observation() else {
        return Err("Wrong boundary event".into());
    };
    assert_eq!(failure.code(), StartupFailureCode::TimedOut);
    assert!(failure.source().is_some_and(|e| e.is::<requests::RequestExpiry>()));
    assert!(c.acquire.ok_or("Original owner absent")?.retiring);
    assert!(c.attempt.is_none());
    Ok(())
}
#[test]
fn acquisition_correlated_foreign_or_earlier_output_expiry_keeps_original_rejection_strict()
-> Result<()> {
    let (mut c, _owner, _clock) = child()?;
    let mut ledger = requests::tests::requests_for_run(c.run)?;
    let original = Moment::from_nanos(100);
    let h = ledger.prepare(original, CounterDeadline::new(100, 100)?)?;
    ledger.acquisition(h, sugarsubstitute_application::startup::ConnectionBudgetKind::Attached)?;
    c.acquire = Some(PendingAcquire {
        kind: sugarsubstitute_application::startup::ConnectionBudgetKind::Attached,
        header: h,
        original,
        admitted: false,
        retiring: false,
    });
    let foreign = ledger.prepare(original, CounterDeadline::new(100, 100)?)?;
    ledger.acquisition(
        foreign,
        sugarsubstitute_application::startup::ConnectionBudgetKind::Attached,
    )?;
    let cause = ledger.expiry(foreign, RequestError::Deadline)?;
    assert!(c.recover_acquisition_expiry(Box::new(cause)).is_err());
    let earlier = Moment::from_nanos(90);
    let output = ledger.prepare(earlier, CounterDeadline::new(90, 100)?)?;
    ledger.output_pull(output)?;
    c.output.pending = Some((output, earlier));
    assert!(
        c.recover_acquisition_expiry(Box::new(ledger.expiry(output, RequestError::Deadline)?))
            .is_err()
    );
    assert!(
        c.recover_acquisition_expiry(Box::new(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "unrelated real infrastructure timeout"
        )))
        .is_err()
    );
    assert!(!c.acquire.ok_or("Original owner absent")?.retiring);
    assert!(c.take_startup_event()?.is_none());
    assert_eq!(c.acquire.ok_or("Original owner absent")?.header, h);
    Ok(())
}

#[test]
fn acquisition_authenticated_denial_requires_actual_current_absence_and_ack_before_settlement()
-> Result<()> {
    let (mut c, _owner, _clock) = child()?;
    c.acquisition_header = Some(queued(&c, Moment::from_nanos(100))?.header);
    c.begin_attempt_cleanup()?;
    assert!(c.shutdown.main_absence_pending);
    assert!(
        matches!(c.take_command()?,Some(DesktopStartupCommand::ConfirmMainAbsent(id)) if id==c.identity)
    );
    assert!(!c.attempt.as_ref().ok_or("Original cleanup lost")?.complete());
    c.observe(DesktopStartupObservation::MainAbsent(c.identity), None)?;
    assert!(c.shutdown.main_retired && !c.shutdown.main_retirement_ack);
    c.advance_attempt()?;
    assert!(!c.attempt.as_ref().ok_or("Original cleanup lost")?.complete());
    assert_eq!(
        c.attempt.as_ref().ok_or("Original cleanup lost")?.remote_state(),
        (false, false, false)
    );
    assert!(c.take_startup_event()?.is_none());
    assert!(
        c.ensure_retry_ready(MainIdentity { launch: LaunchId::new(2)?, ..c.identity }).is_err()
    );
    Ok(())
}
