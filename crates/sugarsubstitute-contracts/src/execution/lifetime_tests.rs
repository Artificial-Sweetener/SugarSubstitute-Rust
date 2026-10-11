// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Verify handle delegation, cancellation distinction and retained terminal charges.

use super::*;
use crate::execution::{
    Generation, IdentityError, JobClass, JobId, JobOutcome, MemoryLease, MemoryRelease, Operation,
    ReplacementKey, SurfaceId,
};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
    time::Duration,
};

#[derive(Default)]
struct Calls {
    cancel: AtomicUsize,
    abandon: AtomicUsize,
    requested: AtomicBool,
    acknowledgements: AtomicUsize,
}

struct ScopeProbe {
    owner: ExecutionId,
    id: ScopeId,
    calls: Arc<Calls>,
}

impl ScopeControl for ScopeProbe {
    fn owner_id(&self) -> ExecutionId {
        self.owner
    }
    fn cancel(&self) {
        self.calls.cancel.fetch_add(1, Ordering::SeqCst);
        self.calls.requested.store(true, Ordering::SeqCst);
    }
    fn snapshot(&self) -> ScopeSnapshot {
        ScopeSnapshot {
            id: self.id,
            closed: false,
            cancellation_requested: self.calls.requested.load(Ordering::SeqCst),
            queued: 1,
            running: 2,
            terminal_unobserved: 3,
            children_live: 1,
            reserved_bytes: 64,
        }
    }
    fn abandon(&self) {
        self.calls.abandon.fetch_add(1, Ordering::SeqCst);
        self.calls.requested.store(true, Ordering::SeqCst);
    }
}

type ProbeCompletion = JobCompletion<u32, &'static str>;
struct JobProbe {
    calls: Arc<Calls>,
    completion: Arc<Mutex<Option<ProbeCompletion>>>,
    phase: JobPhase,
}
impl JobControl<u32, &'static str> for JobProbe {
    fn cancel(&self) {
        self.calls.cancel.fetch_add(1, Ordering::SeqCst);
        self.calls.requested.store(true, Ordering::SeqCst);
    }
    fn status(&self) -> JobStatus {
        JobStatus {
            phase: self.phase,
            cancellation_requested: self.calls.requested.load(Ordering::SeqCst),
        }
    }
    fn try_take_completion(&self) -> Option<ProbeCompletion> {
        let completion =
            self.completion.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take();
        if completion.is_some() {
            self.calls.acknowledgements.fetch_add(1, Ordering::SeqCst);
        }
        completion
    }
    fn abandon(&self) {
        self.calls.abandon.fetch_add(1, Ordering::SeqCst);
        self.calls.requested.store(true, Ordering::SeqCst);
    }
}

struct Release(Arc<AtomicU64>);
impl MemoryRelease for Release {
    fn release(&self, bytes: u64) {
        self.0.fetch_add(bytes, Ordering::SeqCst);
    }
}

fn job_spec() -> Result<JobSpec, IdentityError> {
    Ok(JobSpec {
        id: JobId::new(2)?,
        operation: Operation::FramePrepare,
        class: JobClass::CpuInteractive,
        deadline: None,
        reservation_bytes: 64,
        replacement: Some(ReplacementKey {
            surface: SurfaceId::new(3)?,
            revision: Generation::new(1)?,
        }),
    })
}

#[test]
fn scope_drop_abandons_once_without_claiming_running_jobs_stopped() -> Result<(), IdentityError> {
    let calls = Arc::new(Calls::default());
    let spec = ScopeSpec { id: ScopeId::new(1)?, deadline: None };
    let owner = ExecutionId::new(7)?;
    let scope = Scope::new(spec, Box::new(ScopeProbe { owner, id: spec.id, calls: calls.clone() }));
    assert_eq!(scope.owner_id(), owner);
    assert_eq!(scope.id(), spec.id);
    assert_eq!(scope.spec(), spec);
    scope.cancel();
    let snapshot = scope.snapshot();
    assert!(snapshot.cancellation_requested);
    assert_eq!(snapshot.running, 2);
    assert_eq!(snapshot.queued, 1);
    assert_eq!(snapshot.terminal_unobserved, 3);
    assert_eq!(snapshot.children_live, 1);
    assert_eq!(snapshot.reserved_bytes, 64);
    drop(scope);
    assert_eq!(calls.cancel.load(Ordering::SeqCst), 1);
    assert_eq!(calls.abandon.load(Ordering::SeqCst), 1);
    Ok(())
}

#[test]
fn job_cancel_and_drop_request_cancellation_without_fabricating_terminal_result()
-> Result<(), IdentityError> {
    let calls = Arc::new(Calls::default());
    let mut job = OwnedJob::new(
        ScopeId::new(1)?,
        job_spec()?,
        Box::new(JobProbe {
            calls: calls.clone(),
            completion: Arc::new(Mutex::new(None)),
            phase: JobPhase::Running,
        }),
    );
    assert!(!job.status().cancellation_requested);
    job.cancel();
    assert_eq!(job.status(), JobStatus { phase: JobPhase::Running, cancellation_requested: true });
    assert!(job.try_take_completion().is_none());
    drop(job);
    assert_eq!(calls.cancel.load(Ordering::SeqCst), 1);
    assert_eq!(calls.abandon.load(Ordering::SeqCst), 1);
    assert_eq!(calls.acknowledgements.load(Ordering::SeqCst), 0);
    Ok(())
}

#[test]
fn terminal_transfer_acknowledges_once_and_retained_result_memory_outlives_handle()
-> Result<(), Box<dyn std::error::Error>> {
    let calls = Arc::new(Calls::default());
    let released = Arc::new(AtomicU64::new(0));
    let scope_id = ScopeId::new(1)?;
    let spec = job_spec()?;
    let completion = JobCompletion {
        scope_id,
        spec,
        queue_time: Duration::from_nanos(3),
        execution_time: Duration::from_nanos(5),
        outcome: JobOutcome::Success(19),
        memory: MemoryLease::new(64, Box::new(Release(released.clone()))),
    };
    let mut job = OwnedJob::new(
        scope_id,
        spec,
        Box::new(JobProbe {
            calls: calls.clone(),
            completion: Arc::new(Mutex::new(Some(completion))),
            phase: JobPhase::Terminal(TerminalKind::Success),
        }),
    );
    let completion = job.try_take_completion().ok_or("Missing typed completion")?;
    assert_eq!(completion.scope_id, scope_id);
    assert_eq!(completion.spec, spec);
    assert_eq!(completion.queue_time, Duration::from_nanos(3));
    assert_eq!(completion.execution_time, Duration::from_nanos(5));
    assert!(matches!(completion.outcome, JobOutcome::Success(19)));
    assert!(job.try_take_completion().is_none());
    assert_eq!(calls.acknowledgements.load(Ordering::SeqCst), 1);
    let result_guard = completion.memory.clone();
    drop(completion);
    drop(job);
    assert_eq!(released.load(Ordering::SeqCst), 0);
    drop(result_guard);
    assert_eq!(released.load(Ordering::SeqCst), 64);
    assert_eq!(calls.abandon.load(Ordering::SeqCst), 1);
    Ok(())
}
