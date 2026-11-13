// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Use genuine execution completions; only work failure and clock/wake boundaries are controlled.
use super::*;
use crate::completion_wake::Wake;
use std::sync::Arc;
use sugarsubstitute_execution::ExecutionOwner;
use sugarsubstitute_execution::windows::NativeObservationError;

struct Clock;
impl sugarsubstitute_contracts::execution::Clock for Clock {
    fn now(&self) -> Moment {
        Moment::from_nanos(0)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
fn request(id: u64) -> Result<JobSpec, Box<dyn Error>> {
    Ok(JobSpec {
        id: JobId::new(id)?,
        operation: Operation::Infrastructure,
        class: JobClass::InfrastructureOrdinary,
        deadline: None,
        reservation_bytes: 4096,
        replacement: None,
    })
}

#[test]
fn helper_terminal_failure_preserves_original_source_and_full_guard_after_owner_join()
-> Result<(), Box<dyn Error>> {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(Clock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let spec = request(1)?;
    let mut job = client.submit_blocking(&scope, spec, |_| {
        Err::<(), _>(NativeObservationError::new(
            "original_retirement",
            std::io::Error::other("original external retirement failure"),
        ))
    })?;
    let original = Original::new(&scope, spec);
    let done = wake.completion(&client, &mut job)?;
    assert!(original.matches(&done));
    assert!(size_of::<Terminals>() <= done.memory.bytes() as usize);
    let mut wrong = original;
    wrong.spec.id = JobId::new(2)?;
    assert!(!wrong.matches(&done));
    let failure =
        classify(&done.outcome, true).err().ok_or("Original terminal failure swallowed")?;
    let cause =
        failure.downcast_ref::<NativeObservationError>().ok_or("Original cause type lost")?;
    assert_eq!(cause.stage(), "original_retirement");
    assert_eq!(
        cause.source().ok_or("Original source lost")?.to_string(),
        "original external retirement failure"
    );
    drop(job);
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 4096);
    drop(done);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn helper_terminal_cancellation_requires_real_closing_and_retains_original_identity()
-> Result<(), Box<dyn Error>> {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(Clock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut spec = request(1)?;
    spec.class = JobClass::IoLifecycle;
    spec.operation = Operation::Startup;
    spec.deadline = Some(Moment::from_nanos(1000));
    let mut job = client.submit_async(&scope, spec, |_| async {
        std::future::pending::<()>().await;
        Ok::<_, NativeObservationError>(())
    })?;
    job.cancel();
    let done = wake.completion(&client, &mut job)?;
    assert!(matches!(done.outcome, JobOutcome::Cancelled));
    assert!(Original::new(&scope, spec).matches(&done));
    assert!(classify(&done.outcome, false).is_err());
    classify(&done.outcome, true)?;
    drop(job);
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 4096);
    drop(done);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn pre_auth_helper_pipe_rejects_foreign_key_and_keeps_entire_typed_completion()
-> Result<(), Box<dyn Error>> {
    use sugarsubstitute_contracts::{backend_custody::BackendCustodySession, lifecycle::*};
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(Clock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let key = BackendCustodyKey::new(
        CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        BackendCustodySession::new(1)?,
    );
    let foreign = BackendCustodyKey::new(key.owner(), BackendCustodySession::new(2)?);
    let spec = request(1)?;
    let mut job = client.submit_blocking(&scope, spec, move |_| {
        Ok::<_, LifecyclePipeFailure>(BackendCustodyPipeReport {
            key: Some(foreign),
            received_messages: 0,
            completed_writes: 0,
        })
    })?;
    let done = wake.completion(&client, &mut job)?;
    let mut terminals =
        Terminals { pipe_original: Some(Original::new(&scope, spec)), ..Terminals::default() };
    let failure = terminals
        .pipe(done, false, false, key)
        .err()
        .ok_or("Foreign actual report accepted before authentication")?;
    assert!(matches!(failure.downcast_ref::<Issue>(), Some(Issue::Identity)));
    assert!(
        matches!(&terminals.pipe.as_ref().ok_or("Whole original terminal missing")?.outcome, JobOutcome::Success(report) if report.key == Some(foreign))
    );
    drop(job);
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 4096);
    drop(terminals);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
