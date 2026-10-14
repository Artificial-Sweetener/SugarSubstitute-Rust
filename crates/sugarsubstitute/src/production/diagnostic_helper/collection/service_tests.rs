// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Test actual charged service controls without constructing native Capture inputs.
use super::*;
/// Missing genuine input cannot restore policy or manufacture file/completion proof.
#[test]
fn collection_unsubmitted_missing_native_input_retains_identity_cause_without_proof()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    crate::production::incident_host::tests::admitted_value(
        crate::production::incident_host::CONTROLLER_BYTES,
        |context| {
            let mut service = CollectionService::new(&context)?;
            let mut output = None;
            assert!(service.abandon_unsubmitted(&mut output).is_err());
            assert!(service.outcome().is_none());
            assert!(service.cleanup_outcome().is_none());
            assert!(!service.files_retired());
            assert!(service.unsubmitted_failure().is_none());
            assert!(service.take_passive_restoration_observation().is_none());
            service.identity = Some(JobId::new(0).err().ok_or("invalid identity accepted")?);
            let original = service.unsubmitted_failure().map(std::ptr::from_ref);
            service.installed = true;
            // The input guard still rejects this control-only test state.
            assert!(service.abandon_unsubmitted(&mut output).is_err());
            assert!(output.is_none());
            assert!(service.take_passive_restoration_observation().is_none());
            assert!(service.passive_restored_scope().is_none());
            assert_eq!(service.unsubmitted_failure().map(std::ptr::from_ref), original);
            Ok(service)
        },
    )
}

use std::{
    num::NonZeroUsize,
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};
use sugarsubstitute_execution::ExecutionOwner;
type TestError = Box<dyn std::error::Error + Send + Sync>;
struct TestClock;
impl Clock for TestClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(17)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
#[derive(Default)]
struct Wake {
    value: Mutex<u64>,
    changed: Condvar,
}
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        *self.value.lock().unwrap_or_else(std::sync::PoisonError::into_inner) += 1;
        self.changed.notify_all();
        Ok(())
    }
}
fn completion<T: Send + 'static, E: Send + 'static>(
    client: &ExecutionClient,
    wake: &Wake,
    job: &mut OwnedJob<T, E>,
) -> Result<JobCompletion<T, E>, TestError> {
    loop {
        let observed = *wake.value.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        client.drain_completions(NonZeroUsize::MIN)?;
        if let Some(done) = job.try_take_completion() {
            return Ok(done);
        }
        let (value, timeout) = wake
            .changed
            .wait_timeout_while(
                wake.value.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
                Duration::from_secs(10),
                |value| *value == observed,
            )
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if timeout.timed_out() && *value == observed {
            return Err("actual collection service completion stalled".into());
        }
    }
}
/// Central rejection is genuine; the installed flag tests service control without native input.
#[test]
fn collection_unsubmitted_real_central_cancellation_retains_admission_and_original_charge()
-> Result<(), TestError> {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let original_client = client.clone();
    let bytes = crate::production::incident_host::CONTROLLER_BYTES;
    let mut job = client.submit_blocking(
        &scope,
        JobSpec {
            id: JobId::new(1)?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(Moment::from_nanos(100)),
            reservation_bytes: bytes,
            replacement: None,
        },
        move |context| -> Result<_, TestError> {
            let mut service = CollectionService::new(&context)?;
            let cancelled = original_client
                .create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
            cancelled.cancel();
            service.installed = true;
            let cutoff = Moment::from_nanos(80);
            let jobs = JobSequence::new();
            assert_eq!(
                service.retry(&original_client, &cancelled, &jobs, cutoff),
                Err(CollectionIssue::Admission)
            );
            let original = service.unsubmitted_failure().map(std::ptr::from_ref);
            assert!(original.is_some());
            assert_eq!(service.requested, Some((context.owner_id(), cancelled.id(), cutoff)));
            assert_eq!(
                service.retry(&original_client, &cancelled, &jobs, Moment::from_nanos(81)),
                Err(CollectionIssue::Busy)
            );
            let mut output = None;
            assert!(service.abandon_unsubmitted(&mut output).is_err());
            assert_eq!(service.unsubmitted_failure().map(std::ptr::from_ref), original);
            assert!(service.passive_restored_scope().is_none());
            assert!(service.outcome().is_none());
            assert!(!service.files_retired());
            Ok(service)
        },
    )?;
    let done = completion(&client, &wake, &mut job)?;
    let service = match done.outcome {
        JobOutcome::Success(value) => value,
        JobOutcome::Failure(cause) => return Err(cause),
        _ => return Err("actual charged service constructor stopped".into()),
    };
    drop(done.memory);
    drop(job);
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, bytes);
    drop(service);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
