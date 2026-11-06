// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Route cancellation callbacks through the genuine policy and child recipient with frozen time.
use super::*;
use crate::job_sequence::JobSequence;
use crate::production::child::test_support;
use crate::startup::{ShutdownInitiation, StartupBackend, StartupHost, StartupScopes};
use sugarsubstitute_contracts::{
    execution::{Moment, ScopeId, ScopeSpec},
    lifecycle::ShutdownBudget,
};
use sugarsubstitute_execution::MonotonicClock;

/// A real watchdog observation must not discard a completed, unchanged splash frame.
#[test]
fn native_activity_preserves_completed_splash_publication() -> TestResult {
    use crate::preparation::{FirstFrameContent, PreparationDriver};
    use std::{
        num::NonZeroUsize,
        time::{Duration, Instant},
    };
    use sugarsubstitute_contracts::{
        execution::ExecutionPort,
        presentation::{ConsolePresentation, FrameShape},
    };
    use sugarsubstitute_desktop::StartupWindowContent;
    use sugarsubstitute_splash::{AnimationFrame, Appearance};
    use sugarsubstitute_ui::progress::ProgressFrame;
    let run = test_support::run()?;
    let (child, mut owner, _) = test_support::child()?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(4)?, deadline: None }, None)?;
    let mut content = StartupHost::new(
        run,
        LaunchId::new(1)?,
        child,
        MonotonicClock::new(),
        JobSequence::new(),
        StartupScopes {
            child: ScopeSpec { id: ScopeId::new(1)?, deadline: None },
            render: ScopeSpec { id: ScopeId::new(2)?, deadline: None },
        },
    )?;
    let mut render = PreparationDriver::new(
        "en",
        client.clone(),
        scope,
        JobSequence::new(),
        ConsolePresentation::Native,
        FirstFrameContent::Complete,
    )?;
    render.set_visible(true)?;
    let request = |render: &mut PreparationDriver| {
        render.request(
            FrameShape::new(558, 558, 1.0)?,
            Appearance::demo(),
            AnimationFrame::bootstrap(),
            ProgressFrame { completed: 0.0, activity: None },
            None,
        )
    };
    request(&mut render)?;
    let guard = Instant::now().checked_add(Duration::from_secs(30)).ok_or("Test guard overflow")?;
    loop {
        let batch = client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        if render.consume(&batch)?.is_some() {
            break;
        }
        if Instant::now() >= guard {
            return Err("Initial real splash frame did not complete".into());
        }
        std::hint::spin_loop();
    }
    request(&mut render)?;
    let batch = loop {
        let batch = client.drain_completions(NonZeroUsize::MIN.saturating_add(31))?;
        if !batch.notifications.is_empty() {
            break batch;
        }
        if Instant::now() >= guard {
            return Err("Second real splash frame did not complete".into());
        }
        std::hint::spin_loop();
    };
    content.preparation = Some(render);
    content.prepared = true;
    content.visible = true;
    content.view = Some(crate::startup::view::StartupView::new(
        Presentation::new("en", Theme::Dark, true)?,
        sugarsubstitute_ui::accent::AccentPalette::PRODUCT,
    ));
    assert!(!sugarsubstitute_desktop::WindowContent::animating(&content));
    content.observe(DesktopStartupObservation::NativeActivity {
        run,
        revision: std::num::NonZeroU64::MIN,
        activity: sugarsubstitute_contracts::lifecycle::UiWatchdogActivity::VisibleActive,
    })?;
    let render = content.preparation.as_mut().ok_or("Renderer absent")?;
    assert!(render.consume(&batch)?.is_some(), "Watchdog activity discarded completed pixels");
    assert!(!sugarsubstitute_desktop::WindowContent::animating(&content));
    content.invalidate(true);
    assert!(sugarsubstitute_desktop::WindowContent::animating(&content));
    let render = content.preparation.as_mut().ok_or("Renderer absent")?;
    render.request_retirement()?;
    assert!(render.poll_retirement()?);
    render.finish_retirement()?;
    owner.request_shutdown(Moment::from_nanos(5_000_000_000));
    while !owner.poll_shutdown()?.settled {
        if Instant::now() >= guard {
            return Err("Real execution owner did not settle".into());
        }
        std::hint::spin_loop();
    }
    owner.finish_shutdown()?;
    Ok(())
}

/// Fenced presentation facts own no settlement; only current NativeRetired releases child custody.
#[test]
fn cancelled_splash_routes_through_real_child_without_failure_or_early_settlement() -> TestResult {
    let (_, mut session) = policy(false)?;
    let run = test_support::run()?;
    let (child, owner, _clock) = test_support::child()?;
    let mut content = StartupHost::new(
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
