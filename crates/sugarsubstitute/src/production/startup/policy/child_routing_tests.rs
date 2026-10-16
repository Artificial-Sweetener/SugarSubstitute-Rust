// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Route cancellation callbacks through the genuine policy and child recipient with frozen time.
use super::*;
use crate::job_sequence::JobSequence;
use crate::production::{
    child::{ShutdownInitiation, test_support},
    startup::{ProductionStartup, StartupScopes},
};
use sugarsubstitute_contracts::{
    execution::{Moment, ScopeId, ScopeSpec},
    lifecycle::ShutdownBudget,
};
use sugarsubstitute_execution::MonotonicClock;

/// Fenced presentation facts own no settlement; only current NativeRetired releases child custody.
#[test]
fn cancelled_splash_routes_through_real_child_without_failure_or_early_settlement() -> TestResult {
    let (_, mut session) = policy(false)?;
    let run = test_support::run()?;
    let (child, owner, _clock) = test_support::child()?;
    let mut content = ProductionStartup::new(
        run,
        session.launch(),
        child,
        MonotonicClock::new(),
        JobSequence::new(),
        StartupScopes {
            child: ScopeSpec { id: ScopeId::new(1)?, deadline: None },
            render: ScopeSpec { id: ScopeId::new(2)?, deadline: None },
        },
    )?;
    let launch = session.launch();
    assert!(matches!(
        content
            .policy
            .native(DesktopStartupObservation::SplashPresented(launch), Some(&mut session))?,
        StartupRoute::Effect(StartupEffect::PrepareConnection { .. })
    ));
    assert_eq!(session.phase(), StartupPhase::PreparingConnection);
    assert!(matches!(content.child.begin_shutdown()?, ShutdownInitiation::NoCapture));
    let original = ShutdownBudget::new(Moment::from_nanos(0))?;
    content.child.request_shutdown(original)?;
    assert!(matches!(
        content.policy.close(Some(&mut session))?,
        Some(StartupEffect::CleanupConnection { .. })
    ));
    assert_eq!(session.phase(), StartupPhase::Cancelling);
    assert!(!content.child.poll_shutdown()?);
    let route = content
        .policy
        .native(DesktopStartupObservation::SplashRetired(launch), Some(&mut session))?;
    // Splash disposal preserves the child's original unsettled native custody.
    content.route(route)?;
    assert_eq!(session.phase(), StartupPhase::Cancelling);
    assert!(!content.child.poll_shutdown()?);
    assert_eq!(content.child.shutdown_budget(), Some(original));
    assert!(content.child.take_startup_event()?.is_none());
    let identity = content.policy.identity(&session);
    let receipt = ReadinessReceipt::parse(br#"{"schema_version":1,"launch_id":1,"navigation_generation":1,"outcome":{"status":"ready","evidence":{"frontend_version":"1.0.0","sugarcubes_version":"1.0.0","app_graph_ready":true,"canvas_connected":true,"loading_finished":true,"sugarcubes_registered":true,"sugarcubes_ready":true}}}"#)?;
    for observation in [
        DesktopStartupObservation::SplashPresented(launch),
        DesktopStartupObservation::NavigationStarted(identity),
        DesktopStartupObservation::FrontendReceipt { identity, receipt },
        DesktopStartupObservation::MainRevealed(identity),
    ] {
        let route = content.policy.native(observation, Some(&mut session))?;
        content.route(route)?;
        assert_eq!(session.phase(), StartupPhase::Cancelling);
        assert!(!content.child.poll_shutdown()?);
        assert_eq!(content.child.shutdown_budget(), Some(original));
        assert!(content.child.take_startup_event()?.is_none());
    }
    assert!(
        content
            .policy
            .native(DesktopStartupObservation::SplashRetired(LaunchId::new(2)?), Some(&mut session))
            .is_err()
    );
    let foreign = RunKey { generation: RunGeneration::new(2)?, ..run };
    assert!(
        content
            .policy
            .native(DesktopStartupObservation::NativeRetired(foreign), Some(&mut session))
            .is_err()
    );
    assert!(!content.child.poll_shutdown()?);
    let route =
        content.policy.native(DesktopStartupObservation::NativeRetired(run), Some(&mut session))?;
    assert!(
        matches!(&route, StartupRoute::Native(DesktopStartupObservation::NativeRetired(actual)) if *actual == run)
    );
    content.route(route)?;
    assert!(content.child.poll_shutdown()?);
    assert_eq!(session.phase(), StartupPhase::Cancelling);
    assert_eq!(content.child.shutdown_budget(), Some(original));
    content.child.finish_shutdown()?;
    drop(content);
    drop(owner);
    Ok(())
}
