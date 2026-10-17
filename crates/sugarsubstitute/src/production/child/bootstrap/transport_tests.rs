// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve a real queued-Core failed outcome before pre-UI expiration without creating a pipe peer.
use super::super::{ChildBootstrapIssue, tests::bootstrap};
use super::*;
use crate::production::child::configuration::tests::Rig;
use std::{
    error::Error,
    fmt,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use sugarsubstitute_contracts::execution::{
    ExecutionPort, JobClass, JobId, JobSpec, MemoryClaim, Moment, Operation,
};
use sugarsubstitute_execution::windows::NativeObservationError;

struct FundedFailure {
    dropped: Arc<AtomicUsize>,
    _memory: MemoryClaim,
}
impl fmt::Debug for FundedFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FundedFailure { redacted }")
    }
}
impl fmt::Display for FundedFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Original external transport failed")
    }
}
impl Error for FundedFailure {}
impl Drop for FundedFailure {
    fn drop(&mut self) {
        self.dropped.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn real_failed_completion_surfaces_transport_before_deadline_and_keeps_original_funding()
-> Result<(), Box<dyn Error>> {
    let rig = Rig::new().map_err(|cause| cause as Box<dyn Error>)?;
    let dropped = Arc::new(AtomicUsize::new(0));
    let worker_drop = dropped.clone();
    let mut job = rig.client.submit_blocking(
        &rig.scope,
        JobSpec {
            id: JobId::new(1)?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(Moment::from_nanos(100)),
            reservation_bytes: 8192,
            replacement: None,
        },
        move |context| {
            let memory = context
                .memory()
                .claim(256)
                .map_err(|cause| NativeObservationError::new("fixture_memory", cause))?;
            Err::<(), _>(NativeObservationError::new(
                "captured_parent_peer_mismatch",
                FundedFailure { dropped: worker_drop, _memory: memory },
            ))
        },
    )?;
    let completion = rig.completion(&mut job).map_err(|cause| cause as Box<dyn Error>)?;
    let JobOutcome::Failure(original) = &completion.outcome else {
        return Err("Actual failed worker outcome absent".into());
    };
    let pointer = original.source().map(std::ptr::from_ref);
    let calls = Arc::new(AtomicUsize::new(0));
    let mut child = bootstrap(&calls, None)?;
    let cause = preparation_failure(Some(&completion.outcome), !child.closing && !child.delivered)
        .ok_or("Original transport failure remained hidden until deadline")?;
    assert_eq!(
        child.record(ChildBootstrapIssue::Transport, Box::new(cause)),
        ChildBootstrapIssue::Transport
    );
    assert_eq!(child.issue, Some(ChildBootstrapIssue::Transport));
    assert!(child.closing);
    assert!(!child.capture_started);
    assert!(!child.capture_admitted());
    assert_eq!(calls.load(Ordering::Acquire), 0);
    child.record(
        ChildBootstrapIssue::Deadline,
        Box::new(std::io::Error::other("Later cleanup observation")),
    );
    assert_eq!(child.issue, Some(ChildBootstrapIssue::Transport));
    let retained = child
        .failure_source()
        .and_then(|cause| cause.downcast_ref::<NativeObservationError>())
        .ok_or("Actual typed original transport cause lost")?;
    assert_eq!(retained.stage(), "captured_parent_peer_mismatch");
    assert_eq!(retained.source().map(std::ptr::from_ref), pointer);
    drop((completion, job));
    let Rig { owner, scope, .. } = rig;
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 8192);
    assert_eq!(dropped.load(Ordering::Acquire), 0);
    drop(child);
    assert_eq!(dropped.load(Ordering::Acquire), 1);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn successful_cancelled_and_cleanup_channel_outcomes_do_not_create_preparation_failure() {
    let success = JobOutcome::<(), NativeObservationError>::Success(());
    let cancelled = JobOutcome::<(), NativeObservationError>::Cancelled;
    let timed_out = JobOutcome::<(), NativeObservationError>::TimedOut;
    for outcome in [&success, &cancelled, &timed_out] {
        assert!(preparation_failure(Some(outcome), true).is_none());
    }
    let failed = JobOutcome::<(), NativeObservationError>::Failure(NativeObservationError::new(
        "pipe_completion",
        std::io::Error::from_raw_os_error(109),
    ));
    assert!(preparation_failure(Some(&failed), false).is_none());
    assert!(preparation_failure::<(), NativeObservationError>(None, true).is_none());
}
