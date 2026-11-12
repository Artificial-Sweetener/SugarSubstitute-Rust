// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Consume startup effects through public layered contracts without runtime or native fakes.

use serde_json::json;
use std::{error::Error, path::PathBuf};
use sugarsubstitute_application::startup::{StartupPhase, StartupRejection, StartupSession};
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
    let before =
        (session.phase(), session.launch(), session.navigation(), session.connection_origin());
    let error = session.observe(event(observation)?).err().ok_or("Unexpected admission")?;
    assert_eq!(error.reason(), reason);
    assert_eq!(
        before,
        (session.phase(), session.launch(), session.navigation(), session.connection_origin())
    );
    Ok(())
}

/// Actual first submission and all acknowledgements precede their single next effects.
#[test]
fn successful_startup_preserves_connection_across_normal_retirement() -> TestResult {
    let mut session = session(true)?;
    assert_eq!(session.phase(), StartupPhase::SplashFirstFrame);
    assert!(session.failure().is_none());
    assert!(session.connection_origin().is_none());
    assert_eq!(
        session.observe(event(StartupObservation::SplashPresented)?)?,
        Some(StartupEffect::PrepareConnection { launch: LaunchId::new(4)? })
    );
    assert_eq!(session.phase(), StartupPhase::PreparingConnection);
    assert_eq!(
        session.observe(event(StartupObservation::ConnectionReady(origin()?))?)?,
        Some(StartupEffect::LoadFrontend {
            launch: LaunchId::new(4)?,
            navigation: Generation::new(7)?,
            origin: origin()?
        })
    );
    assert_eq!(session.phase(), StartupPhase::ServerReady);
    rejected(
        &mut session,
        StartupObservation::FrontendReceipt(receipt(4, 7, false)?),
        StartupRejection::InvalidOrder,
    )?;
    assert_eq!(
        session
            .observe(event(StartupObservation::FrontendNavigationStarted(Generation::new(7)?))?)?,
        None
    );
    assert_eq!(session.phase(), StartupPhase::WebviewLoading);
    assert_eq!(
        session.observe(event(StartupObservation::FrontendReceipt(receipt(4, 7, false)?))?)?,
        Some(StartupEffect::RetireSplash { launch: LaunchId::new(4)? })
    );
    assert_eq!(session.phase(), StartupPhase::FrontendReady);
    assert_eq!(session.connection_origin(), Some(origin()?));
    assert_eq!(
        session.observe(event(StartupObservation::SplashRetired)?)?,
        Some(StartupEffect::RevealMain { launch: LaunchId::new(4)? })
    );
    assert_eq!(session.phase(), StartupPhase::RevealingMain);
    rejected(&mut session, StartupObservation::SplashRetired, StartupRejection::InvalidOrder)?;
    assert_eq!(session.observe(event(StartupObservation::MainRevealed)?)?, None);
    assert_eq!(session.phase(), StartupPhase::Running);
    rejected(&mut session, StartupObservation::MainRevealed, StartupRejection::InvalidOrder)?;
    assert_eq!(session.connection_origin(), Some(origin()?));
    rejected(&mut session, StartupObservation::ConnectionSettled, StartupRejection::InvalidOrder)?;
    assert_eq!(
        session.intent(LaunchId::new(4)?, SplashIntent::Cancel),
        Err(StartupRejection::InvalidOrder)
    );
    assert_eq!(
        session.retry(LaunchId::new(5)?, Generation::new(1)?, configuration(true)?),
        Err(StartupRejection::SettlementRequired)
    );
    Ok(())
}

/// Both modes cancel once; observed cleanup, not cancellation alone, authorizes increasing retry.
#[test]
fn cancel_requires_settlement_and_retry_never_reuses_attempt_identity() -> TestResult {
    for managed in [false, true] {
        let mut session = session(managed)?;
        loading(&mut session)?;
        assert_eq!(
            session.intent(LaunchId::new(9)?, SplashIntent::Cancel),
            Err(StartupRejection::ForeignLaunch)
        );
        assert_eq!(session.phase(), StartupPhase::WebviewLoading);
        assert_eq!(
            session.intent(LaunchId::new(4)?, SplashIntent::Cancel)?,
            Some(StartupEffect::CleanupConnection {
                launch: LaunchId::new(4)?,
                mode: if managed {
                    ConnectionCleanup::CancelManagedPreparation
                } else {
                    ConnectionCleanup::DisconnectAttachment
                }
            })
        );
        assert_eq!(session.phase(), StartupPhase::Cancelling);
        assert_eq!(session.intent(LaunchId::new(4)?, SplashIntent::Cancel)?, None);
        rejected(
            &mut session,
            StartupObservation::FrontendReceipt(receipt(4, 7, false)?),
            StartupRejection::AdmissionClosed,
        )?;
        assert_eq!(
            session.retry(LaunchId::new(5)?, Generation::new(1)?, configuration(managed)?),
            Err(StartupRejection::SettlementRequired)
        );
        assert_eq!(
            session
                .observe(StartupEvent::new(
                    LaunchId::new(9)?,
                    StartupObservation::ConnectionSettled
                ))
                .err()
                .ok_or("Foreign settlement")?
                .reason(),
            StartupRejection::ForeignLaunch
        );
        assert_eq!(session.phase(), StartupPhase::Cancelling);
        session.observe(event(StartupObservation::ConnectionSettled)?)?;
        assert_eq!(session.phase(), StartupPhase::Stopped);
        for launch in [3, 4] {
            assert_eq!(
                session.retry(LaunchId::new(launch)?, Generation::new(1)?, configuration(managed)?),
                Err(StartupRejection::LaunchIdentityNotIncreasing)
            );
            assert_eq!(session.phase(), StartupPhase::Stopped);
        }
        session.retry(LaunchId::new(5)?, Generation::new(1)?, configuration(managed)?)?;
        assert_eq!(session.phase(), StartupPhase::SplashFirstFrame);
        assert_eq!(session.launch().get(), 5);
        assert_eq!(session.navigation().get(), 1);
        rejected(
            &mut session,
            StartupObservation::SplashPresented,
            StartupRejection::ForeignLaunch,
        )?;
        assert_eq!(
            session.observe(StartupEvent::new(
                LaunchId::new(5)?,
                StartupObservation::SplashPresented
            ))?,
            Some(StartupEffect::PrepareConnection { launch: LaunchId::new(5)? })
        );
    }
    Ok(())
}

/// Reveal is a requested native operation, with cancel/failure races until acknowledgement.
#[test]
fn main_reveal_requires_acknowledgement_and_remains_cancellable_and_fallible() -> TestResult {
    for fail in [false, true] {
        let mut session = session(false)?;
        loading(&mut session)?;
        session.observe(event(StartupObservation::FrontendReceipt(receipt(4, 7, false)?))?)?;
        assert_eq!(
            session.observe(event(StartupObservation::SplashRetired)?)?,
            Some(StartupEffect::RevealMain { launch: LaunchId::new(4)? })
        );
        assert_eq!(session.phase(), StartupPhase::RevealingMain);
        rejected(&mut session, StartupObservation::SplashRetired, StartupRejection::InvalidOrder)?;
        let foreign = StartupEvent::new(LaunchId::new(9)?, StartupObservation::MainRevealed);
        assert_eq!(
            session.observe(foreign).err().ok_or("Foreign reveal admitted")?.reason(),
            StartupRejection::ForeignLaunch
        );
        assert_eq!(session.phase(), StartupPhase::RevealingMain);
        let effect = if fail {
            session.observe(event(StartupObservation::Failed(StartupFailure::new(
                StartupFailureCode::Presentation,
                Box::new(std::io::Error::other("native show failed")),
            )))?)?
        } else {
            session.intent(LaunchId::new(4)?, SplashIntent::Cancel)?
        };
        assert_eq!(
            effect,
            Some(StartupEffect::CleanupConnection {
                launch: LaunchId::new(4)?,
                mode: ConnectionCleanup::DisconnectAttachment,
            })
        );
        assert_eq!(
            session.phase(),
            if fail { StartupPhase::Failed } else { StartupPhase::Cancelling }
        );
        rejected(&mut session, StartupObservation::MainRevealed, StartupRejection::InvalidOrder)?;
        assert_eq!(
            session.retry(LaunchId::new(5)?, Generation::new(1)?, configuration(false)?),
            Err(StartupRejection::SettlementRequired)
        );
        session.observe(event(StartupObservation::ConnectionSettled)?)?;
        assert_eq!(session.phase(), StartupPhase::Stopped);
    }
    Ok(())
}
