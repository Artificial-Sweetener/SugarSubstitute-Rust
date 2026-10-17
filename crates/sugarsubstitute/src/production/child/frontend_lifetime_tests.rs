// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify frontend liveness, evidence fencing and original cleanup through actual owners.
use super::*;
use crate::production::child::test_support::child;
use std::sync::atomic::Ordering;
use sugarsubstitute_application::startup::{
    SessionFailure, StartupPhase, StartupRejection, StartupSession,
};
use sugarsubstitute_contracts::configuration::{
    ConnectionConfiguration, LoopbackOrigin, Presentation, ProfileId, StartupConfiguration, Theme,
};
use sugarsubstitute_contracts::startup::{
    ReadinessError, ReadinessReceipt, SplashIntent, StartupFailure, StartupFailureCode,
};
/// Advance the actual application session through the acknowledged frontend navigation.
fn loading(child: &ProductionChild) -> Result<StartupSession> {
    let mut s = StartupSession::new(
        child.identity.launch,
        child.identity.navigation,
        StartupConfiguration::new(
            ConnectionConfiguration::Attach(LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?),
            ProfileId::new("frontend-lifetime-test".into())?,
            Presentation::new("en", Theme::Dark, true)?,
        ),
    );
    s.observe(StartupEvent::new(child.identity.launch, StartupObservation::SplashPresented))?;
    s.observe(StartupEvent::new(
        child.identity.launch,
        StartupObservation::ConnectionReady(LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?),
    ))?;
    s.observe(StartupEvent::new(
        child.identity.launch,
        StartupObservation::FrontendNavigationStarted(child.identity.navigation),
    ))?;
    Ok(s)
}
/// Retain the same complete schema-one readiness evidence for actual session admission.
fn ready_wire(child: &ProductionChild) -> String {
    format!(
        r#"{{"schema_version":1,"launch_id":{},"navigation_generation":{},"outcome":{{"status":"ready","evidence":{{"frontend_version":"1.0.0","sugarcubes_version":"1.0.0","app_graph_ready":true,"canvas_connected":true,"loading_finished":true,"sugarcubes_registered":true,"sugarcubes_ready":true}}}}}}"#,
        child.identity.launch.get(),
        child.identity.navigation.get()
    )
}
/// Parse readiness through the public contract before the authoritative session accepts it.
fn ready(child: &ProductionChild) -> Result<ReadinessReceipt> {
    Ok(ReadinessReceipt::parse(ready_wire(child).as_bytes())?)
}
/// Loading owns cancellation and liveness, so elapsed time schedules no aggregate abort.
#[test]
fn frontend_initialization_has_no_elapsed_time_cutoff() -> Result<()> {
    let (mut child, _owner, clock) = child()?;
    child.prepared = true;
    child.phase = Some(StartupPhase::WebviewLoading);
    for elapsed in [0, 30, 180, 3_600, 86_400] {
        clock.0.store(elapsed * 1_000_000_000, Ordering::SeqCst);
        assert_eq!(child.next_deadline(), None, "Unexpected startup abort at {elapsed}s");
        assert!(child.take_startup_event()?.is_none());
        assert!(child.take_command()?.is_none());
    }
    Ok(())
}

/// Genuine readiness remains admissible after lengthy initialization without changing its evidence.
#[test]
fn late_current_frontend_receipt_advances_the_actual_loading_session() -> Result<()> {
    let (mut c, _owner, clock) = child()?;
    c.prepared = true;
    let mut session = loading(&c)?;
    c.phase = Some(session.phase());
    for seconds in [30, 180, 3_600, 86_400] {
        clock.0.store(seconds * 1_000_000_000, Ordering::SeqCst);
        assert_eq!(c.next_deadline(), None);
        assert!(c.take_startup_event()?.is_none());
        assert!(c.take_command()?.is_none());
    }
    let effect = session.observe(StartupEvent::new(
        c.identity.launch,
        StartupObservation::FrontendReceipt(ready(&c)?),
    ))?;
    assert!(
        matches!(effect, Some(StartupEffect::RetireSplash { launch }) if launch == c.identity.launch)
    );
    assert_eq!(session.phase(), StartupPhase::FrontendReady);
    assert!(session.failure().is_none());
    Ok(())
}

/// Elapsed time grants no authority to unknown wire versions or stale navigation receipts.
#[test]
fn late_frontend_schema_and_navigation_stay_strictly_fenced() -> Result<()> {
    let (mut c, _owner, clock) = child()?;
    c.prepared = true;
    let mut session = loading(&c)?;
    c.phase = Some(session.phase());
    clock.0.store(86_400_000_000_000, Ordering::SeqCst);
    let mut value: serde_json::Value = serde_json::from_str(&ready_wire(&c))?;
    value["schema_version"] = 2.into();
    assert!(matches!(
        ReadinessReceipt::parse(value.to_string().as_bytes()),
        Err(ReadinessError::UnsupportedVersion(2))
    ));
    value["schema_version"] = 1.into();
    value["navigation_generation"] = (c.identity.navigation.get() + 1).into();
    let stale = ReadinessReceipt::parse(value.to_string().as_bytes())?;
    let rejection = session
        .observe(StartupEvent::new(c.identity.launch, StartupObservation::FrontendReceipt(stale)))
        .err()
        .ok_or("Stale receipt accepted")?;
    assert_eq!(rejection.reason(), StartupRejection::WrongNavigation);
    assert_eq!(session.phase(), StartupPhase::WebviewLoading);
    assert_eq!(c.next_deadline(), None);
    assert!(c.take_startup_event()?.is_none());
    assert!(session.failure().is_none());
    Ok(())
}

/// Cancellation and actual process failure still close readiness admission at any elapsed time.
#[test]
fn late_frontend_readiness_cannot_override_cancellation_or_process_exit() -> Result<()> {
    for process_exit in [false, true] {
        let (mut c, _owner, clock) = child()?;
        let mut session = loading(&c)?;
        clock.0.store(180_000_000_000, Ordering::SeqCst);
        let effect = if process_exit {
            c.push_event(StartupEvent::new(
                c.identity.launch,
                StartupObservation::Failed(StartupFailure::new(
                    StartupFailureCode::ProcessExited,
                    Box::new(std::io::Error::other("actual process exit")),
                )),
            ))?;
            session.observe(c.take_startup_event()?.ok_or("Original failure missing")?)?
        } else {
            session.intent(c.identity.launch, SplashIntent::Cancel)?
        };
        assert!(matches!(effect, Some(StartupEffect::CleanupConnection { .. })));
        let phase = session.phase();
        let rejected = session
            .observe(StartupEvent::new(
                c.identity.launch,
                StartupObservation::FrontendReceipt(ready(&c)?),
            ))
            .err()
            .ok_or("Closed frontend accepted late readiness")?;
        assert_eq!(rejected.reason(), StartupRejection::AdmissionClosed);
        assert_eq!(session.phase(), phase);
        if process_exit {
            let Some(SessionFailure::Adapter(failure)) = session.failure() else {
                return Err("Original process failure lost".into());
            };
            assert_eq!(failure.code(), StartupFailureCode::ProcessExited);
            assert_eq!(
                failure
                    .source()
                    .and_then(|cause| cause.downcast_ref::<std::io::Error>())
                    .map(ToString::to_string),
                Some("actual process exit".into())
            );
        } else {
            assert_eq!(phase, StartupPhase::Cancelling);
            assert!(session.failure().is_none());
        }
        c.begin_attempt_cleanup()?;
        assert!(!c.attempt.as_ref().ok_or("Cleanup owner absent")?.complete());
        assert!(
            c.ensure_retry_ready(MainIdentity { launch: LaunchId::new(2)?, ..c.identity }).is_err()
        );
    }
    Ok(())
}

/// Cancellation keeps one finite fifteen-second cleanup transaction after arbitrary loading time.
#[test]
fn late_frontend_cleanup_keeps_original_fifteen_seconds_and_native_absence_requirement()
-> Result<()> {
    let (mut c, _owner, clock) = child()?;
    c.prepared = true;
    clock.0.store(86_400_000_000_000, Ordering::SeqCst);
    c.begin_attempt_cleanup()?;
    let original = c.attempt.as_ref().ok_or("Original cleanup absent")?.original_deadline();
    assert_eq!(original, Moment::from_nanos(86_415_000_000_000));
    assert_eq!(c.next_deadline(), Some(original));
    assert!(
        matches!(c.take_command()?, Some(DesktopStartupCommand::ConfirmMainAbsent(id)) if id == c.identity)
    );
    assert!(c.take_startup_event()?.is_none());
    clock.0.store(86_405_000_000_000, Ordering::SeqCst);
    c.begin_attempt_cleanup()?;
    assert_eq!(c.attempt.as_ref().ok_or("Original cleanup lost")?.original_deadline(), original);
    assert!(!c.attempt.as_ref().ok_or("Original cleanup lost")?.complete());
    clock.0.store(original.as_nanos(), Ordering::SeqCst);
    let cause = c.advance_attempt().err().ok_or("Original cleanup expiry ignored")?;
    assert_eq!(
        cause.downcast_ref::<std::io::Error>().map(std::io::Error::kind),
        Some(std::io::ErrorKind::TimedOut)
    );
    assert_eq!(c.attempt.as_ref().ok_or("Cleanup owner lost")?.original_deadline(), original);
    assert!(!c.attempt.as_ref().ok_or("Cleanup owner lost")?.complete());
    Ok(())
}
