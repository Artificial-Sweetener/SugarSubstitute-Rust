// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise actual expiry admission and bounded custody with an injected execution clock.
use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use sugarsubstitute_application::startup::{SessionFailure, StartupSession};
use sugarsubstitute_contracts::configuration::{
    ConnectionConfiguration, LoopbackOrigin, Presentation, ProfileId, StartupConfiguration, Theme,
};
pub(in crate::production::child) struct Frozen(pub(in crate::production::child) AtomicU64);
impl Clock for Frozen {
    fn now(&self) -> Moment {
        Moment::from_nanos(self.0.load(Ordering::SeqCst))
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
struct Wake;
impl CompletionWake for Wake {
    fn wake(&self) -> std::result::Result<(), ExecutionError> {
        Ok(())
    }
}
pub(in crate::production::child) fn child()
-> Result<(ProductionChild, sugarsubstitute_execution::ExecutionOwner, Arc<Frozen>)> {
    let run = RunKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([7; 32]),
            epoch: CoordinatorEpoch::new([8; 16])?,
        },
        instance: InstanceId::new(1)?,
        generation: RunGeneration::new(1)?,
    };
    let clock = Arc::new(Frozen(AtomicU64::new(0)));
    let owner = sugarsubstitute_execution::ExecutionOwner::new(clock.clone(), Arc::new(Wake))?;
    let identity = (LaunchId::new(1)?, Generation::new(1)?);
    let bootstrap = ChildBootstrap::new(
        channel::ChildChannel::new(
            InheritedLifecycleBootstrap::new(1)?,
            LifecyclePipeDeadline::new(100, 100)?,
            run,
        ),
        capture::ChildCapture::new(Box::new(test_support::InertDriver(Arc::default()))),
        ScopeSpec { id: ScopeId::new(3)?, deadline: None },
        identity,
    )?;
    let mut child = ProductionChild::assemble(
        bootstrap,
        run,
        identity,
        MainWindowSettings {
            recovery: sugarsubstitute_desktop::MainRecoveryLabels {
                retry: sugarsubstitute_contracts::localization::messages("en").retry,
                retry_ready: sugarsubstitute_contracts::localization::messages("en").retry_ready,
            },
            logical_width: 1280,
            logical_height: 720,
            dark: true,
            labels: sugarsubstitute_desktop::MainCaptionLabels {
                minimize: "min",
                maximize: "max",
                restore: "restore",
                close: "close",
            },
        },
        crate::diagnostics::DiagnosticHealth::fixture(),
    );
    child.client = Some(owner.client());
    child.frontend_budget = Some(FrontendBudget::new(Moment::from_nanos(0))?);
    Ok((child, owner, clock))
}
fn loading(child: &ProductionChild) -> Result<StartupSession> {
    let mut s = StartupSession::new(
        child.identity.launch,
        child.identity.navigation,
        StartupConfiguration::new(
            ConnectionConfiguration::Attach(LoopbackOrigin::parse("http://127.0.0.1:8188", 8188)?),
            ProfileId::new("frontend-timeout-test".into())?,
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
fn ready(child: &ProductionChild) -> Result<sugarsubstitute_contracts::startup::ReadinessReceipt> {
    Ok(sugarsubstitute_contracts::startup::ReadinessReceipt::parse(format!(r#"{{"schema_version":1,"launch_id":{},"navigation_generation":{},"outcome":{{"status":"ready","evidence":{{"frontend_version":"1.0.0","sugarcubes_version":"1.0.0","app_graph_ready":true,"canvas_connected":true,"loading_finished":true,"sugarcubes_registered":true,"sugarcubes_ready":true}}}}}}"#,child.identity.launch.get(),child.identity.navigation.get()).as_bytes())?)
}
#[test]
fn frontend_timeout_exact_cutoff_publishes_one_original_failure_without_job_or_renewal()
-> Result<()> {
    let (mut c, _owner, clock) = child()?;
    let budget = c.frontend_budget.ok_or("Budget lost")?;
    clock.0.store(budget.until().as_nanos() - 1, Ordering::SeqCst);
    assert!(!c.expire_frontend(StartupPhase::WebviewLoading)?);
    assert!(c.take_startup_event()?.is_none());
    clock.0.store(budget.until().as_nanos(), Ordering::SeqCst);
    assert!(c.expire_frontend(StartupPhase::WebviewLoading)?);
    let event = c.take_startup_event()?.ok_or("Actual timeout not delivered")?;
    assert_eq!(event.launch(), c.identity.launch);
    let mut session = loading(&c)?;
    let effect = session.observe(event)?.ok_or("Actual failure did not request cleanup")?;
    assert!(matches!(effect, StartupEffect::CleanupConnection { .. }));
    let Some(SessionFailure::Adapter(failure)) = session.failure() else {
        return Err("Original failure lost".into());
    };
    assert_eq!(failure.code(), StartupFailureCode::TimedOut);
    let cause = failure
        .source()
        .and_then(|cause| cause.downcast_ref::<std::io::Error>())
        .ok_or("Original typed io cause lost")?;
    assert_eq!(cause.kind(), std::io::ErrorKind::TimedOut);
    assert_eq!(cause.to_string(), "Original frontend loading budget expired");
    assert_eq!(c.frontend_budget, Some(budget));
    assert!(c.acquire.is_none() && !c.main.pending() && c.attempt.is_none());
    assert!(c.expire_frontend(StartupPhase::WebviewLoading)?);
    assert!(c.take_startup_event()?.is_none());
    assert!(session.retry_configured(LaunchId::new(2)?, Generation::new(1)?).is_err());
    Ok(())
}
#[test]
fn frontend_timeout_full_original_event_and_command_slots_keep_cause_and_reject_extra_admission()
-> Result<()> {
    let (mut c, _owner, clock) = child()?;
    for _ in 0..2 {
        c.push_event(StartupEvent::new(c.identity.launch, StartupObservation::ConnectionSettled))?;
    }
    clock.0.store(c.frontend_budget.ok_or("Budget lost")?.until().as_nanos(), Ordering::SeqCst);
    assert!(c.expire_frontend(StartupPhase::WebviewLoading).is_err());
    assert!(!c.frontend_timed_out);
    assert_eq!(c.events.iter().flatten().count(), 2);
    assert_eq!(
        c.failure
            .as_ref()
            .and_then(|cause| cause.downcast_ref::<std::io::Error>())
            .map(std::io::Error::kind),
        Some(std::io::ErrorKind::TimedOut)
    );
    for _ in 0..4 {
        c.push_command(DesktopStartupCommand::Close)?;
    }
    assert!(
        c.push_command(DesktopStartupCommand::RetireMain {
            identity: c.identity,
            reason: sugarsubstitute_desktop::MainRetirementReason::FrontendTimeout
        })
        .is_err()
    );
    assert_eq!(c.commands.iter().flatten().count(), 4);
    assert!(c.attempt.is_none());
    Ok(())
}
#[test]
fn frontend_timeout_queued_receipt_at_cutoff_loses_to_failure_but_accepted_earlier_receipt_stays_ready()
-> Result<()> {
    let (mut c, _owner, clock) = child()?;
    let mut before = loading(&c)?;
    let queued = ready(&c)?;
    clock.0.store(c.frontend_budget.ok_or("Budget lost")?.until().as_nanos(), Ordering::SeqCst);
    assert!(c.expire_frontend(before.phase())?);
    before.observe(c.take_startup_event()?.ok_or("Expiry event lost")?)?;
    let rejected = before
        .observe(StartupEvent::new(c.identity.launch, StartupObservation::FrontendReceipt(queued)))
        .err()
        .ok_or("Queued late receipt granted readiness")?;
    assert_eq!(
        rejected.reason(),
        sugarsubstitute_application::startup::StartupRejection::AdmissionClosed
    );
    assert_eq!(before.phase(), StartupPhase::Failed);
    let (mut early, _owner2, clock2) = child()?;
    let mut session = loading(&early)?;
    clock2.0.store(
        early.frontend_budget.ok_or("Budget lost")?.until().as_nanos() - 1,
        Ordering::SeqCst,
    );
    assert!(!early.expire_frontend(session.phase())?);
    session.observe(StartupEvent::new(
        early.identity.launch,
        StartupObservation::FrontendReceipt(ready(&early)?),
    ))?;
    clock2
        .0
        .store(early.frontend_budget.ok_or("Budget lost")?.until().as_nanos(), Ordering::SeqCst);
    assert!(!early.expire_frontend(session.phase())?);
    assert_eq!(session.phase(), StartupPhase::FrontendReady);
    assert!(early.take_startup_event()?.is_none());
    Ok(())
}

#[test]
fn frontend_timeout_retry_preflight_rejects_unacknowledged_actual_cleanup_and_foreign_identity()
-> Result<()> {
    let (mut c, _owner, clock) = child()?;
    let next = MainIdentity { launch: LaunchId::new(2)?, ..c.identity };
    assert!(c.ensure_retry_ready(next).is_err());
    clock.0.store(
        c.frontend_budget.ok_or("Original cutoff absent")?.until().as_nanos(),
        Ordering::SeqCst,
    );
    c.expire_frontend(StartupPhase::WebviewLoading)?;
    let original_event = c.take_startup_event()?.ok_or("Original timeout absent")?;
    let mut session = loading(&c)?;
    session.observe(original_event)?;
    c.begin_attempt_cleanup()?;
    assert!(
        matches!(c.take_command()?, Some(DesktopStartupCommand::ConfirmMainAbsent(id)) if id == c.identity)
    );
    let original =
        c.attempt.as_ref().ok_or("Partial actual cleanup not retained")?.original_deadline();
    clock.0.store(original.as_nanos() + 1, Ordering::SeqCst);
    c.begin_attempt_cleanup()?;
    assert_eq!(c.attempt.as_ref().ok_or("Cleanup lost")?.original_deadline(), original);
    assert!(c.ensure_retry_ready(next).is_err());
    assert_eq!(session.phase(), StartupPhase::Failed);
    assert!(session.failure().is_some());
    let foreign = MainIdentity {
        run: RunKey { generation: RunGeneration::new(2)?, ..c.identity.run },
        ..next
    };
    assert!(c.ensure_retry_ready(foreign).is_err());
    let before =
        (session.launch(), session.navigation(), session.phase(), c.identity, c.frontend_budget);
    assert!(
        crate::production::startup::ProductionStartup::prepare_retry_policy(&c, &mut session, next)
            .is_err()
    );
    assert_eq!(
        (session.launch(), session.navigation(), session.phase(), c.identity, c.frontend_budget),
        before
    );
    assert!(c.take_startup_event()?.is_none());
    Ok(())
}
