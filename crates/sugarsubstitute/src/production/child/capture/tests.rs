// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise application matching and inert construction without fabricating capture authority.
use super::super::test_support::InertDriver;
use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use sugarsubstitute_contracts::{
    execution::{CompletionNotification, ExecutionId, ScopeId, TerminalKind},
    lifecycle::{CoordinatorEpoch, CoordinatorKey, InstanceId, NamespaceId, RunGeneration, RunKey},
};

type TestResult = Result<(), Box<dyn Error>>;
fn correlation() -> Result<CaptureCorrelation, Box<dyn Error>> {
    Ok(CaptureCorrelation::new(
        RunKey {
            owner: CoordinatorKey {
                namespace: NamespaceId::new([1; 32]),
                epoch: CoordinatorEpoch::new([2; 16])?,
            },
            instance: InstanceId::new(3)?,
            generation: RunGeneration::new(4)?,
        },
        ExecutionId::new(5)?,
        ScopeId::new(6)?,
        JobSpec {
            id: JobId::new(7)?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(sugarsubstitute_contracts::execution::Moment::from_nanos(100)),
            reservation_bytes: 4_194_304,
            replacement: None,
        },
    ))
}
fn notice(
    scope: u64,
    job: u64,
    kind: TerminalKind,
) -> Result<CompletionNotification, Box<dyn Error>> {
    Ok(CompletionNotification {
        scope_id: ScopeId::new(scope)?,
        job_id: JobId::new(job)?,
        kind,
        replacement: None,
    })
}
#[test]
fn construction_and_empty_consumption_do_not_call_the_external_driver() -> TestResult {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut capture = ChildCapture::new(Box::new(InertDriver(calls.clone())));
    capture.consume(&CompletionBatch { notifications: Vec::new(), remaining: 0 })?;
    assert!(!capture.ready());
    assert!(capture.startup().is_none());
    assert!(capture.take_startup().is_none());
    assert!(capture.take_startup().is_none());
    assert!(capture.terminal().is_none());
    assert!(capture.admission_rejection().is_none());
    assert!(capture.cleanup_snapshot().is_none());
    assert!(capture.try_take_process_lifetime().is_none());
    drop(capture);
    assert_eq!(calls.load(Ordering::Acquire), 0);
    Ok(())
}
#[test]
fn cleanup_before_admission_cannot_create_budget_or_native_handoff() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut capture = ChildCapture::new(Box::new(InertDriver(calls.clone())));
    assert!(capture.request_cleanup().is_none());
    assert!(capture.request_cleanup().is_none());
    assert!(!capture.ready());
    assert!(capture.take_startup().is_none());
    assert!(capture.try_take_process_lifetime().is_none());
    drop(capture);
    assert_eq!(calls.load(Ordering::Acquire), 0);
}
#[test]
fn foreign_metadata_does_not_enable_original_completion() -> TestResult {
    let mut actual = correlation()?;
    actual.notice(notice(8, 7, TerminalKind::Success)?)?;
    actual.notice(notice(6, 8, TerminalKind::Success)?)?;
    assert!(!actual.noticed());
    actual.notice(notice(6, 7, TerminalKind::Failure)?)?;
    assert!(actual.noticed());
    Ok(())
}
#[test]
fn conflicting_notice_preserves_first_kind_and_rejects_replacement() -> TestResult {
    let mut actual = correlation()?;
    actual.notice(notice(6, 7, TerminalKind::Failure)?)?;
    assert_eq!(
        actual.notice(notice(6, 7, TerminalKind::Success)?),
        Err(ChildCaptureError::Identity)
    );
    let mut changed = notice(6, 7, TerminalKind::Failure)?;
    changed.replacement = Some(JobId::new(9)?);
    assert_eq!(actual.notice(changed), Err(ChildCaptureError::Identity));
    let original = correlation()?;
    // Matching facts are inert; this creates neither an Attached value nor native custody.
    let spec = JobSpec {
        id: JobId::new(7)?,
        operation: Operation::NativeLifecycle,
        class: JobClass::InfrastructureLifecycle,
        deadline: Some(sugarsubstitute_contracts::execution::Moment::from_nanos(100)),
        reservation_bytes: 4_194_304,
        replacement: None,
    };
    assert!(!original.terminal(ScopeId::new(6)?, spec, TerminalKind::Failure));
    assert!(actual.terminal(ScopeId::new(6)?, spec, TerminalKind::Failure));
    assert!(!actual.terminal(
        ScopeId::new(6)?,
        JobSpec { reservation_bytes: 1, ..spec },
        TerminalKind::Failure
    ));
    assert!(!actual.terminal(ScopeId::new(6)?, spec, TerminalKind::Success));
    Ok(())
}
#[test]
fn source_run_execution_generation_and_job_must_all_match() -> TestResult {
    let actual = correlation()?;
    let mut run = RunKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        instance: InstanceId::new(3)?,
        generation: RunGeneration::new(4)?,
    };
    assert!(actual.attached(run, ExecutionId::new(5)?, JobId::new(7)?));
    assert!(!actual.attached(run, ExecutionId::new(9)?, JobId::new(7)?));
    assert!(!actual.attached(run, ExecutionId::new(5)?, JobId::new(9)?));
    run.generation = RunGeneration::new(9)?;
    assert!(!actual.attached(run, ExecutionId::new(5)?, JobId::new(7)?));
    Ok(())
}
#[test]
fn actual_non_symbol_completion_keeps_original_metadata_without_symbol_ledger() -> TestResult {
    use sugarsubstitute_contracts::execution::{
        Clock, CompletionWake, ExecutionError, ExecutionPort, Moment, ScopeSpec, WaitFuture,
    };
    /// Fake only the external clock; execution owns the real record and completion.
    struct ClockBoundary;
    impl Clock for ClockBoundary {
        fn now(&self) -> Moment {
            Moment::from_nanos(0)
        }
        fn sleep_until(&self, _: Moment) -> WaitFuture {
            Box::pin(std::future::pending())
        }
    }
    /// Coalesce actual completion wakes at the test's external observation boundary.
    struct WakeBoundary {
        notified: std::sync::Mutex<bool>,
        changed: std::sync::Condvar,
    }
    impl CompletionWake for WakeBoundary {
        fn wake(&self) -> Result<(), ExecutionError> {
            let mut notified = self.notified.lock().map_err(|_| {
                ExecutionError::new(
                    Operation::NativeLifecycle,
                    Box::new(std::io::Error::other("Capture test observation poisoned")),
                )
            })?;
            *notified = true;
            self.changed.notify_one();
            Ok(())
        }
    }
    let wake = Arc::new(WakeBoundary {
        notified: std::sync::Mutex::new(false),
        changed: std::sync::Condvar::new(),
    });
    let owner =
        sugarsubstitute_execution::ExecutionOwner::new(Arc::new(ClockBoundary), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(6)?, deadline: None }, None)?;
    let spec = JobSpec {
        id: JobId::new(7)?,
        operation: Operation::NativeLifecycle,
        class: JobClass::InfrastructureLifecycle,
        deadline: Some(Moment::from_nanos(100)),
        reservation_bytes: 4_194_304,
        replacement: None,
    };
    let run = RunKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        instance: InstanceId::new(3)?,
        generation: RunGeneration::new(4)?,
    };
    let mut actual = CaptureCorrelation::new(run, scope.owner_id(), scope.id(), spec);
    let mut job = client.submit_blocking(&scope, spec, |context| {
        if context.native_handles().is_some() {
            return Err(std::io::Error::other("Non-Symbol job received Symbol ledger"));
        }
        Ok(())
    })?;
    let until = std::time::Instant::now()
        .checked_add(std::time::Duration::from_secs(10))
        .ok_or("Capture test observation cutoff overflow")?;
    let mut completed = loop {
        let batch = client.drain_completions(std::num::NonZeroUsize::MIN)?;
        for notice in batch.notifications {
            actual.notice(notice)?;
        }
        if actual.noticed()
            && let Some(value) = job.try_take_completion()
        {
            break value;
        }
        let mut notified = wake.notified.lock().map_err(|_| "Capture test observation poisoned")?;
        while !*notified {
            let remaining = until
                .checked_duration_since(std::time::Instant::now())
                .ok_or("Capture completion did not publish")?;
            let (next, timed) = wake
                .changed
                .wait_timeout(notified, remaining)
                .map_err(|_| "Capture test observation poisoned")?;
            notified = next;
            if timed.timed_out() && !*notified {
                return Err("Capture completion did not publish".into());
            }
        }
        *notified = false;
    };
    assert!(completed.native_handles.is_none());
    assert_eq!(completed.memory.bytes(), 4_194_304);
    assert!(actual.completion(&completed));
    completed.scope_id = ScopeId::new(9)?;
    assert!(!actual.completion(&completed));
    completed.scope_id = scope.id();
    completed.spec.reservation_bytes += 1;
    assert!(!actual.completion(&completed));
    completed.spec = spec;
    completed.outcome = JobOutcome::Failure(std::io::Error::other("Changed outcome"));
    assert!(!actual.completion(&completed));
    drop((completed, job, scope));
    drop(owner);
    Ok(())
}
