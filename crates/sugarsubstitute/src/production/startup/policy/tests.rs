// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise real startup policy with external native/frontend observation fixtures.
use super::*;
use sugarsubstitute_contracts::{
    configuration::{
        ConnectionConfiguration, LaunchConfiguration, LoopbackOrigin, Presentation, ProfileId,
        StartupConfiguration, Theme,
    },
    execution::Generation,
    lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration},
    startup::{ConnectionCleanup, LaunchId, ReadinessReceipt, StartupFailure, StartupFailureCode},
};
type TestResult = Result<(), Box<dyn Error>>;
fn policy(managed: bool) -> Result<(StartupPolicy, StartupSession), Box<dyn Error>> {
    let lexical_root = std::env::temp_dir().join("policy-unit-intent");
    let connection = if managed {
        ConnectionConfiguration::Launch(LaunchConfiguration::new(
            lexical_root.clone(),
            lexical_root.join("python.exe"),
            Vec::new(),
            8188,
        )?)
    } else {
        ConnectionConfiguration::Attach(LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?)
    };
    let run = RunKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        instance: InstanceId::new(1)?,
        generation: RunGeneration::new(1)?,
    };
    let launch = LaunchId::new(1)?;
    let session = StartupSession::new(
        launch,
        Generation::new(1)?,
        StartupConfiguration::new(
            connection,
            ProfileId::new("policy-unit".into())?,
            Presentation::new("en", Theme::Dark, true)?,
        ),
    );
    Ok((StartupPolicy::new(run, launch), session))
}
fn backend_ready(policy: &mut StartupPolicy, session: &mut StartupSession) -> TestResult {
    assert!(matches!(
        policy.native(
            DesktopStartupObservation::SplashPresented(session.launch()),
            Some(&mut *session)
        )?,
        StartupRoute::Effect(StartupEffect::PrepareConnection { .. })
    ));
    assert!(matches!(
        policy.observe(
            StartupEvent::new(
                session.launch(),
                StartupObservation::ConnectionReady(LoopbackOrigin::parse(
                    "http://127.0.0.1:8188",
                    8188
                )?)
            ),
            &mut *session
        )?,
        StartupRoute::Effect(StartupEffect::LoadFrontend { .. })
    ));
    Ok(())
}
fn frontend_ready(policy: &mut StartupPolicy, session: &mut StartupSession) -> TestResult {
    backend_ready(policy, session)?;
    assert!(matches!(
        policy.native(
            DesktopStartupObservation::NavigationStarted(policy.identity(session)),
            Some(&mut *session)
        )?,
        StartupRoute::None
    ));
    // Fake only the external frontend observation; actual parser/policy remain under test.
    let receipt=ReadinessReceipt::parse(br#"{"schema_version":1,"launch_id":1,"navigation_generation":1,"outcome":{"status":"ready","evidence":{"frontend_version":"1.0.0","sugarcubes_version":"1.0.0","app_graph_ready":true,"canvas_connected":true,"loading_finished":true,"sugarcubes_registered":true,"sugarcubes_ready":true}}}"#)?;
    assert!(matches!(
        policy.native(
            DesktopStartupObservation::FrontendReceipt {
                identity: policy.identity(session),
                receipt
            },
            Some(&mut *session)
        )?,
        StartupRoute::Effect(StartupEffect::RetireSplash { .. })
    ));
    Ok(())
}
#[test]
fn connection_effect_waits_for_actual_splash_observation_and_duplicate_is_rejected() -> TestResult {
    let (mut p, mut session) = policy(false)?;
    assert_eq!(session.phase(), StartupPhase::SplashFirstFrame);
    assert!(matches!(
        p.native(DesktopStartupObservation::SplashPresented(session.launch()), Some(&mut session))?,
        StartupRoute::Effect(StartupEffect::PrepareConnection { .. })
    ));
    assert!(
        p.native(DesktopStartupObservation::SplashPresented(session.launch()), Some(&mut session))
            .is_err()
    );
    assert_eq!(session.phase(), StartupPhase::PreparingConnection);
    Ok(())
}
#[test]
fn complete_frontend_handoff_requires_actual_splash_retirement_before_native_reveal() -> TestResult
{
    let (mut p, mut session) = policy(false)?;
    frontend_ready(&mut p, &mut session)?;
    assert!(
        p.native(DesktopStartupObservation::MainRevealed(p.identity(&session)), Some(&mut session))
            .is_err()
    );
    assert_eq!(session.phase(), StartupPhase::FrontendReady);
    assert!(matches!(
        p.native(DesktopStartupObservation::SplashRetired(session.launch()), Some(&mut session))?,
        StartupRoute::Effect(StartupEffect::RevealMain { .. })
    ));
    assert!(matches!(
        p.native(
            DesktopStartupObservation::MainRevealed(p.identity(&session)),
            Some(&mut session)
        )?,
        StartupRoute::None
    ));
    assert_eq!(session.phase(), StartupPhase::Running);
    Ok(())
}
#[test]
fn each_full_run_component_rejects_native_retirement_without_replacing_current_state() -> TestResult
{
    let (mut p, mut session) = policy(false)?;
    let original = p.run();
    let mut variants = [original; 4];
    variants[0].owner.namespace = NamespaceId::new([8; 32]);
    variants[1].owner.epoch = CoordinatorEpoch::new([8; 16])?;
    variants[2].instance = InstanceId::new(8)?;
    variants[3].generation = RunGeneration::new(8)?;
    for run in variants {
        assert!(
            p.native(DesktopStartupObservation::NativeRetired(run), Some(&mut session)).is_err()
        );
        assert_eq!(session.phase(), StartupPhase::SplashFirstFrame);
    }
    assert!(matches!(
        p.native(DesktopStartupObservation::NativeRetired(original), Some(&mut session))?,
        StartupRoute::Native(_)
    ));
    assert_eq!(session.phase(), StartupPhase::SplashFirstFrame);
    Ok(())
}
#[test]
fn stale_main_navigation_cannot_advance_native_navigation_or_browser_authorization() -> TestResult {
    let (mut p, mut session) = policy(false)?;
    backend_ready(&mut p, &mut session)?;
    let mut stale = p.identity(&session);
    stale.navigation = Generation::new(2)?;
    assert!(
        p.native(DesktopStartupObservation::NavigationStarted(stale), Some(&mut session)).is_err()
    );
    assert!(
        p.native(
            DesktopStartupObservation::ControllerPrepared {
                identity: stale,
                browser_process_id: 77
            },
            Some(&mut session)
        )
        .is_err()
    );
    assert_eq!(session.phase(), StartupPhase::ServerReady);
    Ok(())
}
#[test]
fn numeric_browser_observation_is_forwarded_unchanged_without_profile_or_ui_grant() -> TestResult {
    let (mut p, mut session) = policy(false)?;
    backend_ready(&mut p, &mut session)?;
    match p.native(
        DesktopStartupObservation::ControllerPrepared {
            identity: p.identity(&session),
            browser_process_id: 77,
        },
        Some(&mut session),
    )? {
        StartupRoute::Native(DesktopStartupObservation::ControllerPrepared {
            identity,
            browser_process_id,
        }) => {
            assert_eq!(identity, p.identity(&session));
            assert_eq!(browser_process_id, 77);
        }
        _ => return Err("Numeric PID incorrectly became a native grant".into()),
    }
    assert_eq!(session.phase(), StartupPhase::ServerReady);
    Ok(())
}
#[test]
fn close_preserves_connection_mode_and_emits_one_cleanup_request_without_settlement() -> TestResult
{
    for managed in [false, true] {
        let (mut p, mut session) = policy(managed)?;
        let expected = if managed {
            ConnectionCleanup::CancelManagedPreparation
        } else {
            ConnectionCleanup::DisconnectAttachment
        };
        assert!(
            matches!(p.close(Some(&mut session))?,Some(StartupEffect::CleanupConnection {mode,..}) if mode==expected)
        );
        assert!(p.close(Some(&mut session))?.is_none());
        assert!(p.closing());
        assert_eq!(session.phase(), StartupPhase::Cancelling);
    }
    Ok(())
}
#[test]
fn normal_running_close_is_role_retirement_without_a_second_startup_cancel_transition() -> TestResult
{
    let (mut p, mut session) = policy(false)?;
    frontend_ready(&mut p, &mut session)?;
    p.native(DesktopStartupObservation::SplashRetired(session.launch()), Some(&mut session))?;
    p.native(DesktopStartupObservation::MainRevealed(p.identity(&session)), Some(&mut session))?;
    assert!(p.close(Some(&mut session))?.is_none());
    assert!(p.closing());
    assert_eq!(session.phase(), StartupPhase::Running);
    Ok(())
}
#[test]
fn first_native_failure_keeps_typed_source_and_later_rejected_failure_keeps_its_own_cause()
-> TestResult {
    let (mut p, mut session) = policy(false)?;
    let failure = StartupFailure::new(
        StartupFailureCode::Presentation,
        Box::new(std::io::Error::other("first native cause")),
    );
    assert!(matches!(
        p.native(
            DesktopStartupObservation::Failed { identity: Some(p.identity(&session)), failure },
            Some(&mut session)
        )?,
        StartupRoute::Effect(StartupEffect::CleanupConnection { .. })
    ));
    let rejected = p
        .native(
            DesktopStartupObservation::Failed {
                identity: Some(p.identity(&session)),
                failure: StartupFailure::new(
                    StartupFailureCode::Presentation,
                    Box::new(std::io::Error::other("second native cause")),
                ),
            },
            Some(&mut session),
        )
        .err()
        .ok_or("Later failure accepted")?;
    assert_eq!(
        session
            .failure()
            .and_then(Error::source)
            .and_then(Error::source)
            .and_then(|e| e.downcast_ref::<std::io::Error>())
            .ok_or("First source lost")?
            .to_string(),
        "first native cause"
    );
    assert_eq!(
        rejected
            .source()
            .and_then(Error::source)
            .and_then(|e| e.downcast_ref::<std::io::Error>())
            .ok_or("Rejected source lost")?
            .to_string(),
        "second native cause"
    );
    Ok(())
}
#[test]
fn foreign_failure_is_returned_with_original_typed_cause_before_policy_mutation() -> TestResult {
    let (mut p, mut session) = policy(false)?;
    let mut stale = p.identity(&session);
    stale.run.generation = RunGeneration::new(2)?;
    let rejected = p
        .native(
            DesktopStartupObservation::Failed {
                identity: Some(stale),
                failure: StartupFailure::new(
                    StartupFailureCode::Presentation,
                    Box::new(std::io::Error::other("foreign cause")),
                ),
            },
            Some(&mut session),
        )
        .err()
        .ok_or("Foreign failure accepted")?;
    assert_eq!(
        rejected
            .source()
            .and_then(Error::source)
            .and_then(|e| e.downcast_ref::<std::io::Error>())
            .ok_or("Original source lost")?
            .to_string(),
        "foreign cause"
    );
    assert!(session.failure().is_none());
    assert_eq!(session.phase(), StartupPhase::SplashFirstFrame);
    Ok(())
}

#[test]
fn inert_role_identity_without_prepared_session_cannot_admit_splash_or_navigation() -> TestResult {
    let (mut p, session) = policy(false)?;
    assert_eq!(p.launch(), session.launch());
    assert!(p.native(DesktopStartupObservation::SplashPresented(p.launch()), None).is_err());
    assert!(
        p.native(DesktopStartupObservation::NavigationStarted(p.identity(&session)), None).is_err()
    );
    assert!(!p.closing());
    assert_eq!(session.phase(), StartupPhase::SplashFirstFrame);
    Ok(())
}

#[test]
fn partial_start_close_without_session_routes_only_matching_native_retirement() -> TestResult {
    let (mut p, _) = policy(false)?;
    assert!(p.close(None)?.is_none());
    assert!(p.closing());
    let run = p.run();
    let mut stale = run;
    stale.generation = RunGeneration::new(2)?;
    assert!(p.native(DesktopStartupObservation::NativeRetired(stale), None).is_err());
    assert!(matches!(p.native(DesktopStartupObservation::NativeRetired(run), None)?,
        StartupRoute::Native(DesktopStartupObservation::NativeRetired(actual)) if actual == run));
    assert!(p.close(None)?.is_none());
    Ok(())
}

#[test]
fn actual_closing_splash_retirement_is_forwarded_without_revealing_or_advancing_cancelled_session()
-> TestResult {
    let (mut p, mut session) = policy(false)?;
    backend_ready(&mut p, &mut session)?;
    assert!(matches!(p.close(Some(&mut session))?, Some(StartupEffect::CleanupConnection { .. })));
    let launch = session.launch();
    assert!(
        matches!(p.native(DesktopStartupObservation::SplashRetired(launch), Some(&mut session))?,
        StartupRoute::Native(DesktopStartupObservation::SplashRetired(actual)) if actual == launch)
    );
    assert_eq!(session.phase(), StartupPhase::Cancelling);
    Ok(())
}

#[test]
fn pre_surface_failure_keeps_original_cause_without_constructing_a_session() -> TestResult {
    let (mut p, _) = policy(false)?;
    let observation = DesktopStartupObservation::Failed {
        identity: None,
        failure: StartupFailure::new(
            StartupFailureCode::Presentation,
            Box::new(std::io::Error::other("pre-surface-original-cause")),
        ),
    };
    match p.native(observation, None)? {
        StartupRoute::Native(DesktopStartupObservation::Failed { failure, .. }) => {
            assert_eq!(
                failure.source().ok_or("Lost pre-surface cause")?.to_string(),
                "pre-surface-original-cause"
            );
        }
        _ => return Err("Pre-surface failure manufactured application policy".into()),
    }
    Ok(())
}
