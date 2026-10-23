// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise real execution admission and shutdown without native windows or external services.
use super::super::test_support::{Manual, Wake, observe};
use super::*;
use std::sync::atomic::{AtomicU64, Ordering};
fn finish(session: &mut Session, wake: &Wake) -> Result<()> {
    session.request_shutdown()?;
    observe(wake, || session.poll_shutdown())?;
    session.finish_shutdown()?;
    assert!(session.owner.is_none());
    assert!(session.worker.is_none());
    Ok(())
}
#[test]
fn disabled_distribution_retires_all_execution_without_opening_an_installation() -> Result<()> {
    let wake = Arc::new(Wake::default());
    let mut session = Session::new(1024 * 1024, Box::new(|_| Ok(None)));
    session.start(wake.clone())?;
    observe(&wake, || {
        Ok(session.activity(NonZeroUsize::MIN.saturating_add(31))? == HeadlessStep::Shutdown)
    })?;
    assert!(session.result.is_none());
    assert!(session.worker.is_none());
    assert!(session.require_initialization_success().is_err());
    finish(&mut session, &wake)?;
    session.require_initialization_success()?;
    assert!(session.start(wake).is_err());
    assert!(session.finish_shutdown().is_err());
    Ok(())
}
#[test]
fn initialization_failure_preserves_its_cause_and_still_joins_execution() -> Result<()> {
    let wake = Arc::new(Wake::default());
    let mut session = Session::new(
        1024 * 1024,
        Box::new(|_| Err(io::Error::new(io::ErrorKind::InvalidData, "bad compiled feed"))),
    );
    session.start(wake.clone())?;
    let mut failed = false;
    observe(&wake, || match session.activity(NonZeroUsize::MIN.saturating_add(31)) {
        Err(cause) => {
            assert_eq!(
                cause.downcast_ref::<io::Error>().map(io::Error::kind),
                Some(io::ErrorKind::InvalidData)
            );
            assert_eq!(cause.to_string(), "bad compiled feed");
            failed = true;
            Ok(true)
        }
        Ok(_) => Ok(false),
    })?;
    assert!(failed);
    finish(&mut session, &wake)?;
    assert!(session.require_initialization_success().is_err());
    Ok(())
}
#[test]
fn close_before_start_never_runs_factory_or_creates_an_executor() -> Result<()> {
    let calls = Arc::new(AtomicU64::new(0));
    let invoked = calls.clone();
    let mut session = Session::new(
        1024 * 1024,
        Box::new(move |_| {
            invoked.fetch_add(1, Ordering::SeqCst);
            Ok(None)
        }),
    );
    session.request_shutdown()?;
    assert!(session.start(Arc::new(Wake::default())).is_err());
    assert!(session.poll_shutdown()?);
    session.finish_shutdown()?;
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(session.require_initialization_success().is_err());
    Ok(())
}

#[test]
fn early_close_cannot_report_success_from_a_cancelled_or_late_initialization() -> Result<()> {
    let wake = Arc::new(Wake::default());
    let mut session = Session::new(1024 * 1024, Box::new(|_| Ok(None)));
    session.start(wake.clone())?;
    session.request_shutdown()?;
    finish(&mut session, &wake)?;
    assert!(session.require_initialization_success().is_err());
    Ok(())
}

#[test]
fn verification_initialization_receives_its_real_admitted_memory_budget() -> Result<()> {
    let wake = Arc::new(Wake::default());
    let reservation = super::super::probe::PROBE_RESERVATION_BYTES;
    let mut session = Session::new(
        reservation,
        Box::new(move |context| {
            let _claim = context.memory().claim(reservation).map_err(io::Error::other)?;
            Ok(None)
        }),
    );
    session.start(wake.clone())?;
    observe(&wake, || {
        Ok(session.activity(NonZeroUsize::MIN.saturating_add(31))? == HeadlessStep::Shutdown)
    })?;
    assert!(session.require_initialization_success().is_err());
    finish(&mut session, &wake)?;
    session.require_initialization_success()?;
    Ok(())
}
#[test]
fn repeated_close_keeps_the_original_cleanup_deadline_and_observes_initialization() -> Result<()> {
    let wake = Arc::new(Wake::default());
    let clock = Arc::new(Manual::default());
    let mut session = Session::new(1024 * 1024, Box::new(|_| Ok(None)));
    session.clock = clock.clone();
    session.start(wake.clone())?;
    session.request_shutdown()?;
    let cutoff = session.cleanup_until;
    let native = session.next_deadline();
    clock.0.store(1_000_000_000, Ordering::SeqCst);
    session.request_shutdown()?;
    assert_eq!(session.cleanup_until, cutoff);
    assert_eq!(session.next_deadline(), native);
    finish(&mut session, &wake)
}
#[test]
fn expired_campaign_does_not_admit_storage_and_cleanup_uses_a_live_separate_scope() -> Result<()> {
    let wake = Arc::new(Wake::default());
    let clock = Arc::new(Manual::default());
    let mut session = Session::new(
        1024 * 1024,
        Box::new(|_| {
            let bytes = serde_json::to_vec(
                &serde_json::json!({"schema_version":1,"repository":vec![92;32],
            "feed":{"transport":"https","base":"https://updates.example.test/","redirects":[]}}),
            )?;
            crate::updates::configuration(
                &bytes,
                r"E:\unopened\app".into(),
                r"E:\unopened\workspace".into(),
                Duration::from_secs(30),
            )?
            .map(Worker::new)
            .transpose()
        }),
    );
    session.clock = clock.clone();
    session.start(wake.clone())?;
    let until = session.until.ok_or("Campaign cutoff absent")?;
    assert_eq!(
        session.initializing.as_ref().ok_or("Initialization absent")?.spec().deadline,
        Some(until)
    );
    // Observe the constructor directly so no storage phase can race the deliberate clock advance.
    observe(&wake, || {
        session
            .client
            .as_ref()
            .ok_or("Client absent")?
            .drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        if let Some(done) = session.initializing.as_mut().and_then(OwnedJob::try_take_completion) {
            session.initializing = None;
            session.initialized = true;
            let JobOutcome::Success(worker) = done.outcome else {
                return Err("Constructor failed".into());
            };
            session.worker = worker;
            return Ok(true);
        }
        Ok(false)
    })?;
    clock.0.store(901_000_000_000, Ordering::SeqCst);
    let error =
        session.activity(NonZeroUsize::MIN).err().ok_or("Expired campaign admitted storage")?;
    assert_eq!(
        error.downcast_ref::<io::Error>().map(io::Error::kind),
        Some(io::ErrorKind::TimedOut)
    );
    assert_eq!(session.worker.as_ref().ok_or("Worker absent")?.progress(), Progress::Opening);
    assert!(!session.worker.as_ref().ok_or("Worker absent")?.pending());
    session.request_shutdown()?;
    assert!(!session.cleanup.as_ref().ok_or("Cleanup absent")?.snapshot().closed);
    finish(&mut session, &wake)
}

#[test]
fn supervised_panic_keeps_its_diagnostic_and_execution_still_retires() -> Result<()> {
    let wake = Arc::new(Wake::default());
    let mut session = Session::new(
        1024 * 1024,
        Box::new(|_| std::panic::resume_unwind(Box::new("update constructor fault"))),
    );
    session.start(wake.clone())?;
    observe(&wake, || match session.activity(NonZeroUsize::MIN.saturating_add(31)) {
        Err(cause) => {
            assert!(cause.to_string().contains("update constructor fault"));
            Ok(true)
        }
        Ok(_) => Ok(false),
    })?;
    finish(&mut session, &wake)
}

#[test]
fn late_observation_of_actual_settlement_does_not_fabricate_a_cleanup_timeout() -> Result<()> {
    let wake = Arc::new(Wake::default());
    let clock = Arc::new(Manual::default());
    let mut session = Session::new(1024 * 1024, Box::new(|_| Ok(None)));
    session.clock = clock.clone();
    session.start(wake.clone())?;
    observe(&wake, || {
        Ok(session.activity(NonZeroUsize::MIN.saturating_add(31))? == HeadlessStep::Shutdown)
    })?;
    session.request_shutdown()?;
    let cutoff = session.cleanup_until.ok_or("Cleanup cutoff missing")?;
    session.owner.as_mut().ok_or("Execution missing")?.request_shutdown(cutoff);
    session.stopping = true;
    observe(&wake, || {
        session
            .client
            .as_ref()
            .ok_or("Client absent")?
            .drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        Ok(session.owner.as_ref().ok_or("Execution missing")?.poll_shutdown()?.settled)
    })?;
    clock.0.store(16_000_000_000, Ordering::SeqCst);
    assert!(session.poll_shutdown()?);
    session.finish_shutdown()?;
    Ok(())
}
