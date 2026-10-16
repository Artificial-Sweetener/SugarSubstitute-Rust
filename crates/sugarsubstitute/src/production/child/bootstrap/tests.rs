// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify original real scope custody and no-effect rejection before native admission.
use super::super::test_support::InertDriver;
use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicU64, AtomicUsize, Ordering},
};
use sugarsubstitute_contracts::{
    execution::{Clock, CompletionWake, ExecutionError, ExecutionPort, ScopeId, WaitFuture},
    lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey},
};
use sugarsubstitute_execution::{
    ExecutionOwner,
    windows_lifecycle_pipe::{InheritedLifecycleBootstrap, LifecyclePipeDeadline},
};
type TestResult = Result<(), Box<dyn Error>>;
struct FrozenClock;
impl Clock for FrozenClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(0)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
struct Wake;
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        Ok(())
    }
}
/// Control only the external clock; real execution owns every scope and cancellation.
struct ControlledClock(AtomicU64);
impl Clock for ControlledClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(self.0.load(Ordering::Acquire))
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
fn runtime() -> Result<ExecutionOwner, Box<dyn Error>> {
    Ok(ExecutionOwner::new(Arc::new(FrozenClock), Arc::new(Wake))?)
}
fn bootstrap(
    calls: &Arc<AtomicUsize>,
    deadline: Option<Moment>,
) -> Result<ChildBootstrap, Box<dyn Error>> {
    let expected = RunKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        instance: InstanceId::new(1)?,
        generation: RunGeneration::new(1)?,
    };
    let channel = ChildChannel::new(
        InheritedLifecycleBootstrap::new(1)?,
        LifecyclePipeDeadline::new(100, 100)?,
        expected,
    );
    Ok(ChildBootstrap::new(
        channel,
        ChildCapture::new(Box::new(InertDriver(calls.clone()))),
        ScopeSpec { id: ScopeId::new(3)?, deadline },
        (LaunchId::new(1)?, Generation::new(1)?),
    )?)
}
#[test]
fn cancelled_original_scope_rejects_before_independent_native_scope_and_job_identity() -> TestResult
{
    let calls = Arc::new(AtomicUsize::new(0));
    let owner = runtime()?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    scope.cancel();
    let mut child = bootstrap(&calls, None)?;
    let jobs = JobSequence::new();
    assert_eq!(child.start(&client, scope, &jobs), Err(ChildBootstrapIssue::State));
    assert!(child.scope.as_ref().ok_or("Original scope lost")?.snapshot().cancellation_requested);
    assert!(child.native_scope.is_none());
    assert_eq!(jobs.allocate()?.get(), 1);
    assert!(!child.channel.registered());
    assert!(child.take_prepared_startup().is_none());
    assert_eq!(calls.load(Ordering::Acquire), 0);
    drop(child);
    drop(owner);
    Ok(())
}
#[test]
fn close_before_start_retains_actual_scope_without_native_work_or_cleanup_budget() -> TestResult {
    let calls = Arc::new(AtomicUsize::new(0));
    let owner = runtime()?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut child = bootstrap(&calls, None)?;
    child.close_initialization();
    assert_eq!(child.start(&client, scope, &JobSequence::new()), Err(ChildBootstrapIssue::State));
    assert!(child.scope.is_some());
    assert!(child.native_scope.is_none());
    assert!(child.shutdown_budget().is_none());
    assert!(child.scope().is_some());
    assert!(child.native_scope().is_none());
    assert!(child.original_deadline().is_none());
    assert!(!child.projection_pending());
    assert!(!child.capture_admitted());
    assert!(!child.capture_pending());
    assert!(child.capture_cleanup_snapshot().is_none());
    assert!(child.try_take_process_lifetime().is_none());
    assert!(child.channel_mut().received_header()?.is_none());
    assert!(!child.channel_mut().admitted());
    child.release_source_after_native_retired()?;
    assert!(child.failure_source().is_none());
    assert!(child.take_prepared_startup().is_none());
    assert_eq!(calls.load(Ordering::Acquire), 0);
    drop(child);
    drop(owner);
    Ok(())
}
#[test]
fn persistent_channel_deadline_is_rejected_inertly() -> TestResult {
    let calls = Arc::new(AtomicUsize::new(0));
    assert!(bootstrap(&calls, Some(Moment::from_nanos(100))).is_err());
    assert_eq!(calls.load(Ordering::Acquire), 0);
    Ok(())
}

/// Exercise the actual owner's passive deadline view without native readiness authority.
#[test]
fn quiet_pre_ui_owner_keeps_original_cutoff_until_ready_or_shutdown() -> TestResult {
    use crate::production::child::ProductionChild;
    use sugarsubstitute_desktop::{MainCaptionLabels, MainWindowSettings};
    let calls = Arc::new(AtomicUsize::new(0));
    let mut bootstrap = bootstrap(&calls, None)?;
    let original = Moment::from_nanos(100);
    // This inert cached boundary value grants no admitted capture or transport proof.
    bootstrap.original = Some(original);
    let run = RunKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        instance: InstanceId::new(1)?,
        generation: RunGeneration::new(1)?,
    };
    let mut child = ProductionChild::assemble(
        bootstrap,
        run,
        (LaunchId::new(1)?, Generation::new(1)?),
        MainWindowSettings {
            recovery: sugarsubstitute_desktop::MainRecoveryLabels {
                retry: sugarsubstitute_contracts::localization::messages("en").retry,
                retry_ready: sugarsubstitute_contracts::localization::messages("en").retry_ready,
            },
            logical_width: 1280,
            logical_height: 720,
            dark: true,
            labels: MainCaptionLabels {
                minimize: "minimize",
                maximize: "maximize",
                restore: "restore",
                close: "close",
            },
        },
        crate::diagnostics::DiagnosticHealth::fixture(),
    );
    assert_eq!(child.next_deadline(), Some(original));
    assert_eq!(child.next_deadline(), Some(original));
    assert!(child.bootstrap.take_prepared_startup().is_none());
    // Test only this passive phase flag's deadline selection; publish no Ready or UI.
    child.prepared = true;
    assert_eq!(child.next_deadline(), None);
    child.prepared = false;
    assert_eq!(child.next_deadline(), Some(original));
    assert_eq!(child.begin_shutdown()?, crate::production::child::ShutdownInitiation::NoCapture);
    assert_eq!(child.next_deadline(), None);
    let budget = ShutdownBudget::new(Moment::from_nanos(200))?;
    child.request_shutdown(budget)?;
    assert_eq!(child.next_deadline(), Some(budget.settle_until));
    assert_eq!(calls.load(Ordering::Acquire), 0);
    Ok(())
}

/// Emit only changed closed facts while retaining the first actual supplied cause.
#[test]
fn preparation_diagnostics_suppress_repeats_and_retain_redacted_original_failure() -> TestResult {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut child = bootstrap(&calls, None)?;
    let initial = child.changed_preparation().ok_or("Initial observation missing")?;
    assert_eq!(initial.channel_phase, HandshakePhase::Unadmitted);
    assert_eq!(initial.capture_input, CaptureInput::Unchecked);
    assert!(!initial.capture_admitted);
    assert!(!initial.capture && !initial.registered && !initial.configuration);
    assert_eq!(initial.original_failure, None);
    assert!(!initial.failure_retained);
    assert!(child.changed_preparation().is_none());
    assert!(child.channel.take_captured_startup().is_none());
    let waiting = child.changed_preparation().ok_or("Actual no-peer poll observation missing")?;
    assert_eq!(waiting.channel_phase, HandshakePhase::Unadmitted);
    assert_eq!(waiting.capture_input, CaptureInput::NoPeer);
    assert!(child.changed_preparation().is_none());
    child.record(ChildBootstrapIssue::Deadline, Box::new(std::io::Error::other("private-path")));
    let failure = child.changed_preparation().ok_or("Failure transition missing")?;
    assert!(!failure.capture_admitted);
    assert_eq!(failure.original_failure, Some(ChildBootstrapIssue::Deadline));
    assert!(failure.failure_retained);
    assert!(!format!("{failure:?}").contains("private-path"));
    assert!(child.changed_preparation().is_none());
    child.record(ChildBootstrapIssue::Transport, Box::new(std::io::Error::other("later")));
    assert!(child.changed_preparation().is_none());
    assert_eq!(child.issue, Some(ChildBootstrapIssue::Deadline));
    assert_eq!(child.failure_source().ok_or("Original failure lost")?.to_string(), "private-path");
    assert_eq!(calls.load(Ordering::Acquire), 0);
    Ok(())
}

/// Quiet expiry and actual scope cancellation latch the original diagnostic phase.
#[test]
fn current_scope_and_clock_failure_latch_first_issue_without_synthetic_cause() -> TestResult {
    let calls = Arc::new(AtomicUsize::new(0));
    let clock = Arc::new(ControlledClock(AtomicU64::new(0)));
    let owner = ExecutionOwner::new(clock.clone(), Arc::new(Wake))?;
    let client = owner.client();
    for (id, first) in [(1, ChildBootstrapIssue::Deadline), (2, ChildBootstrapIssue::State)] {
        let scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(id)?, deadline: None }, None)?;
        let mut child = bootstrap(&calls, None)?;
        // Retain genuine application scope/client without admitting the inert native input.
        child.scope = Some(scope);
        child.client = Some(client.clone());
        child.original = Some(Moment::from_nanos(id * 100));
        assert!(child.check_current().is_ok());
        assert_eq!(
            child.changed_preparation().ok_or("Initial observation missing")?.original_failure,
            None
        );
        if first == ChildBootstrapIssue::State {
            child.scope().ok_or("Original scope missing")?.cancel();
        }
        clock.0.store(id * 100, Ordering::Release);
        assert_eq!(child.check_current(), Err(first));
        let observation = child.changed_preparation().ok_or("Failure transition missing")?;
        assert_eq!(observation.original_failure, Some(first));
        assert!(!observation.failure_retained);
        assert!(child.failure_source().is_none());
        assert!(child.closing);
        assert_eq!(child.check_current(), Err(first));
        assert!(child.changed_preparation().is_none());
        child.scope().ok_or("Original scope missing")?.cancel();
        assert_eq!(child.check_current(), Err(ChildBootstrapIssue::State));
        assert_eq!(child.issue, Some(first));
        assert!(child.changed_preparation().is_none());
        assert!(!child.capture_admitted());
        assert_eq!(calls.load(Ordering::Acquire), 0);
        drop(child);
    }
    drop(owner);
    Ok(())
}

/// Typed capture causes remain intact while closed diagnostics discard arbitrary payloads.
#[test]
fn capture_failure_kind_preserves_first_identity_and_redacts_native_payload() -> TestResult {
    use crate::production::child::capture::ChildCaptureError;
    use sugarsubstitute_execution::windows::NativeObservationError;
    let calls = Arc::new(AtomicUsize::new(0));
    let mut identity = bootstrap(&calls, None)?;
    identity.record(ChildBootstrapIssue::Capture, Box::new(ChildCaptureError::Identity));
    let first = identity.changed_preparation().ok_or("Capture transition missing")?;
    assert_eq!(first.capture_failure, CaptureFailureKind::Identity);
    assert!(identity.failure_source().is_some_and(|cause| cause.is::<ChildCaptureError>()));
    identity.record(ChildBootstrapIssue::Capture, Box::new(ChildCaptureError::State));
    assert!(identity.changed_preparation().is_none());
    assert_eq!(identity.capture_failure_kind(), CaptureFailureKind::Identity);
    let mut native = bootstrap(&calls, None)?;
    native.record(
        ChildBootstrapIssue::Capture,
        Box::new(CaptureAttachFailure::Native(NativeObservationError::new(
            "private-stage",
            std::io::Error::other("private-path"),
        ))),
    );
    let observation = native.changed_preparation().ok_or("Native transition missing")?;
    assert_eq!(observation.capture_failure, CaptureFailureKind::Native);
    assert!(observation.failure_retained);
    assert!(!format!("{observation:?}").contains("private-"));
    assert!(native.failure_source().is_some_and(|cause| cause.is::<CaptureAttachFailure>()));
    assert_eq!(calls.load(Ordering::Acquire), 0);
    Ok(())
}

#[test]
fn closed_preparation_failure_survives_failed_diagnostic_write_without_native_effects() -> TestResult
{
    use crate::production::child::ProductionChild;
    use sugarsubstitute_desktop::{MainCaptionLabels, MainWindowSettings};
    let calls = Arc::new(AtomicUsize::new(0));
    let mut bootstrap = bootstrap(&calls, None)?;
    bootstrap.record(
        ChildBootstrapIssue::Capture,
        Box::new(std::io::Error::other("original capture boundary")),
    );
    let run = RunKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        instance: InstanceId::new(1)?,
        generation: RunGeneration::new(1)?,
    };
    let mut child = ProductionChild::assemble(
        bootstrap,
        run,
        (LaunchId::new(1)?, Generation::new(1)?),
        MainWindowSettings {
            recovery: sugarsubstitute_desktop::MainRecoveryLabels {
                retry: sugarsubstitute_contracts::localization::messages("en").retry,
                retry_ready: sugarsubstitute_contracts::localization::messages("en").retry_ready,
            },
            logical_width: 1280,
            logical_height: 720,
            dark: true,
            labels: MainCaptionLabels {
                minimize: "minimize",
                maximize: "maximize",
                restore: "restore",
                close: "close",
            },
        },
        crate::diagnostics::DiagnosticHealth::fixture_failure(std::io::Error::from_raw_os_error(
            232,
        )),
    );
    let failure = child
        .observe_diagnostics::<()>(Err(Box::new(ChildBootstrapIssue::Capture)))
        .err()
        .ok_or("Real diagnostic rejection missing")?;
    assert_eq!(
        child.bootstrap.preparation_observation().original_failure,
        Some(ChildBootstrapIssue::Capture)
    );
    assert_eq!(
        child.bootstrap.failure_source().ok_or("Original cause lost")?.to_string(),
        "original capture boundary"
    );
    assert!(failure.to_string().contains("Capture"));
    assert!(!failure.to_string().contains("original capture boundary"));
    assert_eq!(child.diagnostics.native_error(), Some(232));
    assert_eq!(calls.load(Ordering::Acquire), 0);
    assert!(child.take_prepared_startup().is_none());
    Ok(())
}
