// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Original controller memory and real delivery remain distinct from physical-close flags.
use super::*;
use std::{
    num::NonZeroUsize,
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};
use sugarsubstitute_execution::ExecutionOwner;
type TestError = Box<dyn std::error::Error + Send + Sync>;
type TestResult = Result<(), TestError>;
struct TestClock;
impl Clock for TestClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(1)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
#[derive(Default)]
struct Wake {
    count: Mutex<u64>,
    changed: Condvar,
}
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        *self.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner) += 1;
        self.changed.notify_all();
        Ok(())
    }
}
fn wait(wake: &Wake, before: u64) -> TestResult {
    let (count, timed) = wake
        .changed
        .wait_timeout_while(
            wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
            Duration::from_secs(10),
            |count| *count == before,
        )
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if timed.timed_out() && *count == before {
        return Err("original export test completion stalled".into());
    }
    Ok(())
}
fn complete<T, E>(
    client: &ExecutionClient,
    wake: &Wake,
    job: &mut OwnedJob<T, E>,
) -> Result<JobCompletion<T, E>, TestError> {
    loop {
        let before = *wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        client.drain_completions(NonZeroUsize::MIN)?;
        if let Some(completion) = job.try_take_completion() {
            return Ok(completion);
        }
        wait(wake, before)?;
    }
}
fn spec(id: u64, bytes: u64) -> Result<JobSpec, IdentityError> {
    Ok(JobSpec {
        id: JobId::new(id)?,
        operation: sugarsubstitute_contracts::execution::Operation::Infrastructure,
        class: JobClass::InfrastructureOrdinary,
        deadline: Some(Moment::from_nanos(100)),
        reservation_bytes: bytes,
        replacement: None,
    })
}
#[test]
fn export_metadata_is_inside_original_controller_credit_until_actual_disposal() -> TestResult {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut job = client.submit_blocking(&scope, spec(1, 2_097_152)?, |context| {
        let service = ExportService::new(&context)?;
        let other = context.memory().claim(2_097_152 - EXPORT_SOURCE_BYTES)?;
        assert_eq!(
            context
                .memory()
                .claim(1)
                .err()
                .ok_or("unexpected uncharged controller byte")?
                .available_bytes,
            0
        );
        assert!(service.receipt().is_none());
        Ok::<_, TestError>((service, other))
    })?;
    let completion = complete(&client, &wake, &mut job)?;
    let JobOutcome::Success((mut service, other)) = completion.outcome else {
        return Err("controller worker failed".into());
    };
    drop(completion.memory);
    assert!(service.take_terminal().is_none());
    assert_eq!(scope.snapshot().reserved_bytes, 2_097_152);
    drop(other);
    assert_eq!(scope.snapshot().reserved_bytes, 2_097_152);
    drop(service);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn export_delivery_requires_actual_matching_terminal_then_retains_original_memory() -> TestResult {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let job = client.submit_blocking(&scope, spec(1, 131_072)?, |_| Ok::<_, ExportIssue>(true))?;
    let mut delivery = Delivery { job: Some(job), ..Delivery::default() };
    assert!(!delivery.succeeded());
    loop {
        let before = *wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let batch = client.drain_completions(NonZeroUsize::MIN)?;
        delivery.capture(&batch)?;
        if delivery.completion.is_some() {
            break;
        }
        wait(&wake, before)?;
    }
    assert!(delivery.succeeded());
    assert!(delivery.job.is_none());
    assert_eq!(scope.snapshot().terminal_unobserved, 0);
    assert_eq!(scope.snapshot().reserved_bytes, 131_072);
    drop(delivery);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn export_capture_feeds_other_real_deliveries_after_first_lane_identity_failure() -> TestResult {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut controller = client
        .submit_blocking(&scope, spec(1, 2_097_152)?, |context| ExportService::new(&context))?;
    let completion = complete(&client, &wake, &mut controller)?;
    let JobOutcome::Success(mut service) = completion.outcome else {
        return Err("controller source admission failed".into());
    };
    drop(completion.memory);
    for (index, delivery) in service.delivery.iter_mut().enumerate() {
        delivery.job =
            Some(client.submit_blocking(&scope, spec(index as u64 + 2, 131_072)?, |_| {
                Ok::<_, ExportIssue>(true)
            })?);
    }
    let observed = |batch: &CompletionBatch| {
        service
            .delivery
            .iter()
            .filter(|delivery| {
                delivery.job.as_ref().is_some_and(|job| {
                    batch.notifications.iter().any(|notice| {
                        (notice.scope_id, notice.job_id) == (job.scope_id(), job.spec().id)
                    })
                })
            })
            .count()
    };
    let maximum = NonZeroUsize::new(8).ok_or("completion bound")?;
    let mut batch = client.drain_completions(maximum)?;
    while observed(&batch) < 4 {
        let before = *wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let arrived = client.drain_completions(maximum)?;
        assert!(batch.notifications.len() + arrived.notifications.len() <= 5);
        batch.notifications.extend(arrived.notifications);
        batch.remaining = arrived.remaining;
        if observed(&batch) == 4 {
            break;
        }
        wait(&wake, before)?;
    }
    assert_eq!(observed(&batch), 4);
    // Drain the coalesced wake mailbox; every accumulated notice remains an actual owner output.
    // An inconsistent external notice history is a rejection fixture, never a completion proof.
    service.delivery[0].terminal = Some(TerminalKind::Failure);
    assert_eq!(service.capture(&batch), Err(ExportIssue::Identity));
    for delivery in &service.delivery[1..] {
        assert!(delivery.succeeded());
    }
    assert_eq!(service.delivery_error, Some(ExportIssue::Identity));
    assert!(service.receipt().is_none());
    assert!(service.delivery[0].job.is_some());
    let batch = client.drain_completions(NonZeroUsize::MIN)?;
    assert_eq!(service.capture(&batch), Err(ExportIssue::Identity));
    assert!(service.delivery[0].completion.is_some());
    assert!(service.receipt().is_none());
    drop(service);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[path = "tests/lifecycle.rs"]
mod lifecycle;
