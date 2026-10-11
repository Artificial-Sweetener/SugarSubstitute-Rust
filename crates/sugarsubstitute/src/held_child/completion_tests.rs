// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise the real consumer with deterministic admitted-job boundary fakes.
use super::failure::{ChildFailure, terminal};
use super::*;
use std::{
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};
use sugarsubstitute_contracts::execution::{
    CompletionBatch, CompletionNotification, JobCompletion, JobControl, JobOutcome, JobPhase,
    JobStatus, MemoryLease, MemoryRelease, Moment, OwnedJob, PanicCause, TerminalKind,
};
use sugarsubstitute_execution::{
    windows::{NativeCorrelation, NativeObservationError},
    windows_fixture::FixtureChildReport,
};

fn correlation() -> Result<NativeCorrelation> {
    Ok(NativeCorrelation::new(7)?)
}
fn report() -> Result<FixtureChildReport> {
    Ok(FixtureChildReport {
        correlation: correlation()?,
        inheritance_cleared: true,
        ready_signalled: true,
        response_signalled: false,
        release_observed: true,
    })
}
struct Release(Arc<AtomicUsize>);
impl MemoryRelease for Release {
    fn release(&self, _: u64) {
        self.0.fetch_add(1, Ordering::AcqRel);
    }
}
struct JobBoundary {
    ready: AtomicBool,
    cancelled: AtomicUsize,
    acknowledged: AtomicUsize,
    completion: Mutex<Option<JobCompletion<FixtureChildReport, NativeObservationError>>>,
}
struct JobPort(Arc<JobBoundary>);
impl JobControl<FixtureChildReport, NativeObservationError> for JobPort {
    fn cancel(&self) {
        self.0.cancelled.fetch_add(1, Ordering::AcqRel);
    }
    fn status(&self) -> JobStatus {
        JobStatus {
            phase: if self.0.ready.load(Ordering::Acquire) {
                JobPhase::Terminal(TerminalKind::Success)
            } else {
                JobPhase::Running
            },
            cancellation_requested: self.0.cancelled.load(Ordering::Acquire) != 0,
        }
    }
    fn try_take_completion(
        &self,
    ) -> Option<JobCompletion<FixtureChildReport, NativeObservationError>> {
        if !self.0.ready.load(Ordering::Acquire) {
            return None;
        }
        let completion = match self.0.completion.lock() {
            Ok(mut completion) => completion.take(),
            Err(poisoned) => poisoned.into_inner().take(),
        };
        if completion.is_some() {
            self.0.acknowledged.fetch_add(1, Ordering::AcqRel);
        }
        completion
    }
    fn abandon(&self) {}
}
struct Harness {
    state: ChildCompletion,
    boundary: Arc<JobBoundary>,
    released: Arc<AtomicUsize>,
    notice: CompletionNotification,
}
impl Harness {
    fn new() -> Result<Self> {
        let scope = ScopeId::new(1)?;
        let spec = JobSpec {
            id: JobId::new(1)?,
            operation: Operation::NativeLifecycle,
            class: JobClass::NativeObservation,
            deadline: Some(Moment::from_nanos(1234)),
            reservation_bytes: MIN_FIXTURE_SESSION_BYTES,
            replacement: None,
        };
        let released = Arc::new(AtomicUsize::new(0));
        // This fake admission owns one test charge before constructing its guard;
        // production consumes only the execution adapter's admitted completion.
        let memory =
            MemoryLease::new(MIN_FIXTURE_SESSION_BYTES, Box::new(Release(released.clone())));
        let boundary = Arc::new(JobBoundary {
            ready: AtomicBool::new(false),
            cancelled: AtomicUsize::new(0),
            acknowledged: AtomicUsize::new(0),
            completion: Mutex::new(Some(JobCompletion {
                scope_id: scope,
                spec,
                queue_time: Duration::ZERO,
                execution_time: Duration::ZERO,
                outcome: JobOutcome::Success(report()?),
                memory,
            })),
        });
        let mut state = ChildCompletion::new(correlation()?);
        state.install(OwnedJob::new(scope, spec, Box::new(JobPort(boundary.clone()))))?;
        Ok(Self {
            state,
            boundary,
            released,
            notice: CompletionNotification {
                scope_id: scope,
                job_id: spec.id,
                kind: TerminalKind::Success,
                replacement: None,
            },
        })
    }
    fn batch(&self) -> CompletionBatch {
        CompletionBatch { notifications: vec![self.notice], remaining: 0 }
    }
}

#[test]
fn exact_metadata_acks_once_only_after_actual_typed_terminal_is_available() -> Result<()> {
    let mut h = Harness::new()?;
    h.state.consume(&h.batch())?;
    assert!(!h.state.terminal_consumed());
    assert_eq!(h.boundary.acknowledged.load(Ordering::Acquire), 0);
    assert_eq!(h.released.load(Ordering::Acquire), 0);
    h.boundary.ready.store(true, Ordering::Release);
    h.state.consume(&CompletionBatch { notifications: vec![], remaining: 0 })?;
    assert!(h.state.report().is_some());
    assert!(h.state.terminal_consumed());
    assert_eq!(h.boundary.acknowledged.load(Ordering::Acquire), 1);
    assert_eq!(h.released.load(Ordering::Acquire), 1);
    h.state.consume(&h.batch())?;
    assert_eq!(h.boundary.acknowledged.load(Ordering::Acquire), 1);
    Ok(())
}

#[test]
fn foreign_scope_or_job_metadata_does_not_consume_ready_result() -> Result<()> {
    for scope_changed in [false, true] {
        let mut h = Harness::new()?;
        h.boundary.ready.store(true, Ordering::Release);
        let mut notice = h.notice;
        if scope_changed {
            notice.scope_id = ScopeId::new(2)?
        } else {
            notice.job_id = JobId::new(2)?
        }
        h.state.consume(&CompletionBatch { notifications: vec![notice], remaining: 0 })?;
        assert!(!h.state.terminal_consumed());
        assert_eq!(h.boundary.acknowledged.load(Ordering::Acquire), 0);
        h.state.consume(&h.batch())?;
        assert!(h.state.report().is_some());
    }
    Ok(())
}

#[test]
fn cancellation_requests_once_and_cannot_authorize_finish_without_terminal_and_workers()
-> Result<()> {
    let mut h = Harness::new()?;
    assert!(!h.state.shutdown_settled(true));
    assert!(h.state.request_close());
    assert!(!h.state.request_close());
    assert_eq!(h.boundary.cancelled.load(Ordering::Acquire), 1);
    assert!(!h.state.shutdown_settled(true));
    h.boundary.ready.store(true, Ordering::Release);
    h.state.consume(&h.batch())?;
    assert!(!h.state.shutdown_settled(false));
    assert!(h.state.shutdown_settled(true));
    Ok(())
}

#[test]
fn derived_admitted_deadline_is_retained_through_shutdown_without_rebasing() -> Result<()> {
    let mut h = Harness::new()?;
    h.state.request_close();
    assert_eq!(h.state.shutdown_deadline(Moment::from_nanos(0)), Moment::from_nanos(1234));
    assert_eq!(h.state.shutdown_deadline(Moment::from_nanos(u64::MAX)), Moment::from_nanos(1234));
    h.boundary.ready.store(true, Ordering::Release);
    h.state.consume(&h.batch())?;
    assert_eq!(h.state.shutdown_deadline(Moment::from_nanos(9999)), Moment::from_nanos(1234));
    let unbound = ChildCompletion::new(correlation()?);
    assert_eq!(unbound.shutdown_deadline(Moment::from_nanos(42)), Moment::from_nanos(42));
    Ok(())
}

#[test]
fn original_deadline_expiry_reports_once_and_late_actual_settlement_remains_owned() -> Result<()> {
    let mut h = Harness::new()?;
    h.state.request_close();
    assert!(h.state.observe_shutdown(false, true).is_err());
    assert!(!h.state.observe_shutdown(false, true)?);
    assert!(!h.state.observe_shutdown(true, false)?);
    h.boundary.ready.store(true, Ordering::Release);
    h.state.consume(&h.batch())?;
    assert!(!h.state.observe_shutdown(false, true)?);
    assert!(h.state.observe_shutdown(true, false)?);
    assert_eq!(h.state.shutdown_deadline(Moment::from_nanos(9999)), Moment::from_nanos(1234));
    Ok(())
}

#[test]
fn response_is_optional_but_each_required_native_fact_and_correlation_is_checked() -> Result<()> {
    let full = report()?;
    assert!(terminal::<std::io::Error>(correlation()?, JobOutcome::Success(full)).is_ok());
    for invalid in [
        FixtureChildReport { inheritance_cleared: false, ..full },
        FixtureChildReport { ready_signalled: false, ..full },
        FixtureChildReport { release_observed: false, ..full },
        FixtureChildReport { correlation: NativeCorrelation::new(8)?, ..full },
    ] {
        assert!(matches!(
            terminal::<std::io::Error>(correlation()?, JobOutcome::Success(invalid)),
            Err(ChildFailure::Protocol)
        ));
    }
    assert!(
        terminal::<std::io::Error>(
            correlation()?,
            JobOutcome::Success(FixtureChildReport { response_signalled: true, ..full })
        )
        .is_ok()
    );
    Ok(())
}

#[test]
fn native_cause_panic_cancellation_timeout_and_coalescing_remain_distinct() -> Result<()> {
    let error = terminal(correlation()?, JobOutcome::Failure(std::io::Error::from_raw_os_error(5)))
        .err()
        .ok_or("Missing native error")?;
    assert_eq!(
        error
            .source()
            .and_then(|source| source.downcast_ref::<std::io::Error>())
            .and_then(std::io::Error::raw_os_error),
        Some(5)
    );
    let panic = terminal::<std::io::Error>(
        correlation()?,
        JobOutcome::Panicked(PanicCause::new("PRIVATE_PANIC".into())),
    )
    .err()
    .ok_or("Missing panic")?;
    assert!(matches!(&panic, ChildFailure::Panicked(cause) if cause.0.as_str() == "PRIVATE_PANIC"));
    assert!(!panic.to_string().contains("PRIVATE_PANIC"));
    assert!(!format!("{panic:?}").contains("PRIVATE_PANIC"));
    assert!(matches!(
        terminal::<std::io::Error>(correlation()?, JobOutcome::Cancelled),
        Err(ChildFailure::Cancelled)
    ));
    assert!(matches!(
        terminal::<std::io::Error>(correlation()?, JobOutcome::TimedOut),
        Err(ChildFailure::TimedOut)
    ));
    let replacement = JobId::new(2)?;
    assert!(
        matches!(terminal::<std::io::Error>(correlation()?, JobOutcome::Coalesced { replacement }), Err(ChildFailure::Coalesced(id)) if id == replacement)
    );
    Ok(())
}

#[test]
fn malformed_metadata_kind_and_oversized_batch_never_produce_success() -> Result<()> {
    let mut h = Harness::new()?;
    assert!(
        h.state
            .consume(&CompletionBatch { notifications: vec![h.notice; 33], remaining: 0 })
            .is_err()
    );
    assert_eq!(h.boundary.acknowledged.load(Ordering::Acquire), 0);
    h.boundary.ready.store(true, Ordering::Release);
    let mut notice = h.notice;
    notice.kind = TerminalKind::Cancelled;
    assert!(
        h.state.consume(&CompletionBatch { notifications: vec![notice], remaining: 0 }).is_err()
    );
    assert!(h.state.terminal_consumed());
    assert!(h.state.report().is_none());
    assert_eq!(h.boundary.acknowledged.load(Ordering::Acquire), 1);
    Ok(())
}
