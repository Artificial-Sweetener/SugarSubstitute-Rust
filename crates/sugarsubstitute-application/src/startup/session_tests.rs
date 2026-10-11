// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise ordered application behavior with real configuration and receipt boundaries.

use super::*;
use serde_json::json;
use std::{
    error::Error,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use sugarsubstitute_contracts::{
    configuration::{
        ConnectionConfiguration, LaunchConfiguration, LoopbackOrigin, Presentation, ProfileId,
        StartupConfiguration, Theme,
    },
    execution::Generation,
    startup::{
        ConnectionCleanup, LaunchId, ReadinessReceipt, SplashIntent, StartupEffect, StartupEvent,
        StartupFailure, StartupFailureCode, StartupObservation,
    },
};

type TestResult = Result<(), Box<dyn Error>>;

/// Use lexical fixture paths only; no process, file, network or native surface is opened.
fn configuration(managed: bool) -> Result<StartupConfiguration, Box<dyn Error>> {
    let connection = if managed {
        let (root, interpreter) = if cfg!(windows) {
            (r"X:\fixture\comfy", r"X:\fixture\python.exe")
        } else {
            ("/fixture/comfy", "/fixture/python")
        };
        ConnectionConfiguration::Launch(LaunchConfiguration::new(
            PathBuf::from(root),
            PathBuf::from(interpreter),
            vec!["--example".to_owned()],
            8188,
        )?)
    } else {
        ConnectionConfiguration::Attach(origin()?)
    };
    Ok(StartupConfiguration::new(
        connection,
        ProfileId::new("isolated-test".to_owned())?,
        Presentation::new("en", Theme::Dark, true)?,
    ))
}

fn origin() -> Result<LoopbackOrigin, Box<dyn Error>> {
    Ok(LoopbackOrigin::parse("http://localhost:8188", 8188)?)
}

fn session(managed: bool) -> Result<StartupSession, Box<dyn Error>> {
    Ok(StartupSession::new(LaunchId::new(4)?, Generation::new(7)?, configuration(managed)?))
}

fn event(observation: StartupObservation) -> Result<StartupEvent, Box<dyn Error>> {
    Ok(StartupEvent::new(LaunchId::new(4)?, observation))
}

fn receipt(launch: u64, navigation: u64, failed: bool) -> Result<ReadinessReceipt, Box<dyn Error>> {
    let outcome = if failed {
        json!({"status":"failed","failure":{"code":"timed_out","detail":"bounded diagnostic"}})
    } else {
        json!({"status":"ready","evidence":{
            "frontend_version":"1.47.11","sugarcubes_version":"0.15.0",
            "app_graph_ready":true,"canvas_connected":true,"loading_finished":true,
            "sugarcubes_registered":true,"sugarcubes_ready":true}})
    };
    Ok(ReadinessReceipt::parse(&serde_json::to_vec(&json!({"schema_version":1,
        "launch_id":launch,"navigation_generation":navigation,"outcome":outcome}))?)?)
}

fn loading(session: &mut StartupSession) -> TestResult {
    session.observe(event(StartupObservation::SplashPresented)?)?;
    session.observe(event(StartupObservation::ConnectionReady(origin()?))?)?;
    session.observe(event(StartupObservation::FrontendNavigationStarted(Generation::new(7)?))?)?;
    Ok(())
}

fn rejected(
    session: &mut StartupSession,
    observation: StartupObservation,
    reason: StartupRejection,
) -> TestResult {
    let before = (
        session.phase(),
        session.launch(),
        session.navigation(),
        session.connection_origin(),
        session.configuration().clone(),
    );
    let rejection = session.observe(event(observation)?).err().ok_or("Unexpected admission")?;
    assert_eq!(rejection.reason(), reason);
    assert_eq!(
        before,
        (
            session.phase(),
            session.launch(),
            session.navigation(),
            session.connection_origin(),
            session.configuration().clone()
        )
    );
    Ok(())
}

/// Early/duplicate acknowledgements cannot begin connection work or reveal the main surface.
#[test]
fn invalid_ordering_and_duplicate_acknowledgements_leave_state_unchanged() -> TestResult {
    let mut session = session(false)?;
    rejected(&mut session, StartupObservation::MainRevealed, StartupRejection::InvalidOrder)?;
    rejected(
        &mut session,
        StartupObservation::ConnectionReady(origin()?),
        StartupRejection::InvalidOrder,
    )?;
    rejected(&mut session, StartupObservation::SplashRetired, StartupRejection::InvalidOrder)?;
    rejected(&mut session, StartupObservation::ConnectionSettled, StartupRejection::InvalidOrder)?;
    session.observe(event(StartupObservation::SplashPresented)?)?;
    rejected(&mut session, StartupObservation::SplashPresented, StartupRejection::InvalidOrder)?;
    session.observe(event(StartupObservation::ConnectionReady(origin()?))?)?;
    rejected(
        &mut session,
        StartupObservation::ConnectionReady(origin()?),
        StartupRejection::InvalidOrder,
    )?;
    rejected(
        &mut session,
        StartupObservation::FrontendNavigationStarted(Generation::new(8)?),
        StartupRejection::WrongNavigation,
    )?;
    session.observe(event(StartupObservation::FrontendNavigationStarted(Generation::new(7)?))?)?;
    rejected(
        &mut session,
        StartupObservation::FrontendNavigationStarted(Generation::new(7)?),
        StartupRejection::InvalidOrder,
    )?;
    session.observe(event(StartupObservation::FrontendReceipt(receipt(4, 7, false)?))?)?;
    rejected(
        &mut session,
        StartupObservation::FrontendReceipt(receipt(4, 7, true)?),
        StartupRejection::AlreadyCompleted,
    )?;
    session.observe(event(StartupObservation::SplashRetired)?)?;
    rejected(&mut session, StartupObservation::SplashRetired, StartupRejection::InvalidOrder)?;
    Ok(())
}

/// Readiness must identify the nested current launch/navigation and configured endpoint.
#[test]
fn foreign_stale_future_and_wrong_endpoint_inputs_do_not_consume_admission() -> TestResult {
    for managed in [false, true] {
        let mut session = session(managed)?;
        let foreign = StartupEvent::new(LaunchId::new(9)?, StartupObservation::SplashPresented);
        assert_eq!(
            session.observe(foreign).err().ok_or("Foreign admission")?.reason(),
            StartupRejection::ForeignLaunch
        );
        assert_eq!(session.phase(), StartupPhase::SplashFirstFrame);
        session.observe(event(StartupObservation::SplashPresented)?)?;
        rejected(
            &mut session,
            StartupObservation::ConnectionReady(LoopbackOrigin::parse(
                "http://localhost:8189",
                8189,
            )?),
            StartupRejection::WrongOrigin,
        )?;
        if !managed {
            rejected(
                &mut session,
                StartupObservation::ConnectionReady(LoopbackOrigin::parse(
                    "http://127.0.0.1:8188",
                    8188,
                )?),
                StartupRejection::WrongOrigin,
            )?;
        }
        session.observe(event(StartupObservation::ConnectionReady(origin()?))?)?;
        session
            .observe(event(StartupObservation::FrontendNavigationStarted(Generation::new(7)?))?)?;
        rejected(
            &mut session,
            StartupObservation::FrontendReceipt(receipt(9, 7, false)?),
            StartupRejection::ForeignLaunch,
        )?;
        for navigation in [6, 8] {
            rejected(
                &mut session,
                StartupObservation::FrontendReceipt(receipt(4, navigation, false)?),
                StartupRejection::WrongNavigation,
            )?;
        }
        assert!(matches!(
            session.observe(event(StartupObservation::FrontendReceipt(receipt(4, 7, false)?))?)?,
            Some(StartupEffect::RetireSplash { .. })
        ));
    }
    Ok(())
}

/// Cancel can beat first submission or retirement; neither late acknowledgement can start/reveal.
#[test]
fn cancellation_wins_first_frame_and_retirement_races() -> TestResult {
    for before_first_frame in [false, true] {
        let mut session = session(false)?;
        if !before_first_frame {
            loading(&mut session)?;
            session.observe(event(StartupObservation::FrontendReceipt(receipt(4, 7, false)?))?)?;
        }
        session.intent(LaunchId::new(4)?, SplashIntent::Cancel)?;
        rejected(
            &mut session,
            if before_first_frame {
                StartupObservation::SplashPresented
            } else {
                StartupObservation::SplashRetired
            },
            StartupRejection::InvalidOrder,
        )?;
        rejected(
            &mut session,
            StartupObservation::FrontendReceipt(receipt(4, 7, false)?),
            StartupRejection::AdmissionClosed,
        )?;
        session.observe(event(StartupObservation::ConnectionSettled)?)?;
        rejected(
            &mut session,
            StartupObservation::ConnectionSettled,
            StartupRejection::InvalidOrder,
        )?;
    }
    Ok(())
}

/// This deliberately non-Clone error proves rejection and retained failure own the same cause.
#[derive(Debug)]
struct Cause(Arc<AtomicUsize>);
impl std::fmt::Display for Cause {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("sensitive adapter diagnostic")
    }
}
impl Error for Cause {}
impl Drop for Cause {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

/// Foreign and late failures are returned intact; the accepted original survives settlement.
#[test]
fn adapter_failure_ownership_is_preserved_without_clone_or_replacement() -> TestResult {
    let drops = Arc::new(AtomicUsize::new(0));
    let mut session = session(true)?;
    let foreign = StartupEvent::new(
        LaunchId::new(9)?,
        StartupObservation::Failed(StartupFailure::new(
            StartupFailureCode::ProcessExited,
            Box::new(Cause(drops.clone())),
        )),
    );
    let rejection = session.observe(foreign).err().ok_or("Foreign failure admitted")?;
    assert_eq!(rejection.reason(), StartupRejection::ForeignLaunch);
    assert_eq!(rejection.event().launch().get(), 9);
    assert!(
        rejection.source().and_then(Error::source).ok_or("Rejected source lost")?.is::<Cause>()
    );
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    let failure = match rejection.into_event().into_observation() {
        StartupObservation::Failed(failure) => failure,
        _ => return Err("Failure observation lost".into()),
    };
    assert!(failure.source().ok_or("Cause lost")?.is::<Cause>());
    assert_eq!(failure.code(), StartupFailureCode::ProcessExited);
    assert!(!failure.to_string().contains("sensitive"));
    assert!(matches!(
        session.observe(event(StartupObservation::Failed(failure))?)?,
        Some(StartupEffect::CleanupConnection {
            mode: ConnectionCleanup::CancelManagedPreparation,
            ..
        })
    ));
    assert_eq!(session.phase(), StartupPhase::Failed);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert!(
        session
            .failure()
            .ok_or("Primary failure lost")?
            .source()
            .and_then(Error::source)
            .ok_or("Original cause lost")?
            .is::<Cause>()
    );
    rejected(
        &mut session,
        StartupObservation::Failed(StartupFailure::new(
            StartupFailureCode::TimedOut,
            Box::new(std::io::Error::other("secondary")),
        )),
        StartupRejection::InvalidOrder,
    )?;
    session.intent(LaunchId::new(4)?, SplashIntent::Cancel)?;
    session.observe(event(StartupObservation::ConnectionSettled)?)?;
    assert!(session.failure().is_some());
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    session.retry(LaunchId::new(5)?, Generation::new(1)?, configuration(false)?)?;
    assert!(session.failure().is_none());
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    Ok(())
}

/// A valid failed probe consumes completion and permanently closes admission before cleanup.
#[test]
fn frontend_failure_retains_bounded_detail_and_closes_readiness_before_settlement() -> TestResult {
    let mut session = session(false)?;
    loading(&mut session)?;
    assert_eq!(
        session.observe(event(StartupObservation::FrontendReceipt(receipt(4, 7, true)?))?)?,
        Some(StartupEffect::CleanupConnection {
            launch: LaunchId::new(4)?,
            mode: ConnectionCleanup::DisconnectAttachment
        })
    );
    assert_eq!(session.phase(), StartupPhase::Failed);
    assert!(matches!(session.failure(),Some(SessionFailure::Frontend(failure))
        if failure.detail == "bounded diagnostic"));
    assert!(!session.failure().ok_or("Probe failure lost")?.to_string().contains("diagnostic"));
    rejected(
        &mut session,
        StartupObservation::FrontendReceipt(receipt(4, 7, false)?),
        StartupRejection::AdmissionClosed,
    )?;
    rejected(&mut session, StartupObservation::SplashRetired, StartupRejection::InvalidOrder)?;
    assert_eq!(
        session.retry(LaunchId::new(5)?, Generation::new(1)?, configuration(false)?),
        Err(StartupRejection::SettlementRequired)
    );
    session.observe(event(StartupObservation::ConnectionSettled)?)?;
    assert!(session.failure().is_some());
    Ok(())
}
