// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Real incomplete/failed preparation cannot create native retirement or accepted disposal.
use super::*;
use crate::completion_wake::wait_while_watched;
use std::sync::{Condvar, Mutex};
#[derive(Default)]
struct QueueBoundary {
    state: Mutex<(usize, bool)>,
    changed: Condvar,
}
impl QueueBoundary {
    /// Worker side: count entry, then hold until the test releases every queued worker.
    fn hold(&self) {
        let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.0 += 1;
        self.changed.notify_all();
        drop(wait_while_watched(
            &self.changed,
            state,
            || "the test to release a held finite worker".to_owned(),
            |state| !state.1,
        ));
    }
    fn wait_all(&self, workers: usize) {
        drop(wait_while_watched(
            &self.changed,
            self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
            || format!("{workers} finite workers to reach the controlled external boundary"),
            |state| state.0 != workers,
        ));
    }
    fn release(&self) {
        self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner).1 = true;
        self.changed.notify_all();
    }
}
struct ReleaseQueue(Arc<QueueBoundary>);
impl Drop for ReleaseQueue {
    fn drop(&mut self) {
        self.0.release();
    }
}
#[test]
fn final_reporter_observation_waits_for_actual_queued_completion_before_natural_close() -> Result<()>
{
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let gate = Arc::new(QueueBoundary::default());
    let _release = ReleaseQueue(gate.clone());
    let workers = owner.limits().blocking_workers;
    let mut blockers = Vec::new();
    for index in 0..workers {
        let mut specification = spec()?;
        specification.id = JobId::new(u64::try_from(index)? + 1)?;
        specification.reservation_bytes = 0;
        let boundary = gate.clone();
        blockers.push(client.submit_blocking(&scope, specification, move |_| {
            boundary.hold();
            Ok::<(), std::io::Error>(())
        })?);
    }
    gate.wait_all(workers);
    let mut specification = spec()?;
    specification.id = JobId::new(u64::try_from(workers)? + 1)?;
    let until = specification.deadline.ok_or("Original final completion cutoff absent")?;
    let execution = client.clone();
    let mut delivery = delivery::Delivery::new();
    delivery.job = Some(client.submit_blocking(&scope, specification, move |context| {
        delivery::prepare_counter(&execution, until, &context)
    })?);
    let job = delivery.job.as_ref().ok_or("Actual queued completion missing")?;
    assert_eq!(job.status().phase, JobPhase::Queued);
    let mut receiving = IncidentReporting::new();
    let request = DiagnosticRequestId::new(7)?;
    // Passive Host phase input, not a fabricated authenticated frame or native retirement proof.
    receiving.native_retirement.receive(request)?;
    assert!(!receiving.native_close_ready());
    if receiving.native_close_ready() {
        job.cancel();
    }
    assert!(!job.status().cancellation_requested);
    gate.release();
    while delivery.job.is_some() {
        delivery.capture(&batch(&client, &wake)?)?;
    }
    assert!(!receiving.native_close_ready());
    assert!(delivery.value()?.is_some());
    assert!(matches!(
        delivery.completion.as_ref().map(|done| &done.outcome),
        Some(JobOutcome::Success(_))
    ));
    // Only the same accepted completion chooses the no-further-read disposition.
    assert!(receiving.native_retirement.consume_completion(request, true)?);
    assert!(receiving.native_close_ready());
    assert_eq!(delivery.completion.as_ref().map(|done| done.spec.deadline), Some(Some(until)));
    for mut blocker in blockers {
        loop {
            if let Some(done) = blocker.try_take_completion() {
                assert!(matches!(done.outcome, JobOutcome::Success(())));
                break;
            }
            batch(&client, &wake)?;
        }
    }
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 16 * 1024);
    drop(delivery);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn missing_rejected_or_foreign_final_completion_never_admits_natural_close() -> Result<()> {
    let request = DiagnosticRequestId::new(7)?;
    let foreign = DiagnosticRequestId::new(8)?;
    let mut receiving = IncidentReporting::new();
    assert!(!receiving.native_retirement.consume_completion(request, true)?);
    assert!(!receiving.native_close_ready());
    receiving.native_retirement.receive(request)?;
    assert!(receiving.native_retirement.receive(request).is_err());
    for (actual, accepted) in [(request, false), (foreign, true)] {
        assert!(receiving.native_retirement.consume_completion(actual, accepted).is_err());
        assert_eq!(receiving.native_retirement, NativeRetirement::Received(request));
        assert!(!receiving.native_close_ready());
    }
    assert!(receiving.native_retirement.consume_completion(request, true)?);
    assert!(receiving.native_close_ready());
    assert!(receiving.native_retirement.consume_completion(request, true).is_err());
    Ok(())
}
fn inert_binding() -> Result<ReportBinding> {
    use sugarsubstitute_contracts::lifecycle::*;
    let run = RunKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        instance: InstanceId::new(3)?,
        generation: RunGeneration::new(4)?,
    };
    Ok(ReportBinding::new(
        IncidentKey::new(IncidentSubject::Desktop(run)),
        IncidentRevision::new(5)?,
        ReportSessionId::new(6)?,
    ))
}
#[test]
fn retirement_phase_actual_unobserved_preparation_cannot_issue_recycle_or_disposal() -> Result<()> {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut receiving = IncidentReporting::new();
    receiving.receiving_session.preparing_mut()?.metadata.job =
        Some(client.submit_blocking(&scope, spec()?, |context| {
            Err(delivery::Failure::test_cause(&context, "Original phase failure"))
        })?);
    assert!(
        receiving
            .receiving_session
            .retire_healthy_preparation(
                inert_binding()?,
                DiagnosticKey::new(
                    inert_binding()?.incident().subject().coordinator(),
                    DiagnosticRole::Reporter,
                    DiagnosticSessionId::new(inert_binding()?.session().get())?
                )
            )
            .is_err()
    );
    assert!(receiving.receiving_session.preparing()?.metadata.job.is_some());
    assert!(receiving.receiving_session.recycled().is_none());
    assert!(receiving.receiving_session.retired().is_none());
    assert!(receiving.receiving_session.validate_disposal().is_err());
    assert!(receiving.normal_close_budget().is_none());
    receiving.capture(&batch(&client, &wake)?)?;
    assert!(
        receiving
            .receiving_session
            .retire_healthy_preparation(
                inert_binding()?,
                DiagnosticKey::new(
                    inert_binding()?.incident().subject().coordinator(),
                    DiagnosticRole::Reporter,
                    DiagnosticSessionId::new(inert_binding()?.session().get())?
                )
            )
            .is_err()
    );
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 16 * 1024);
    drop(receiving);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn retirement_phase_repeated_failure_denial_keeps_same_cause_and_original_delivery() -> Result<()> {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut receiving = IncidentReporting::new();
    receiving.receiving_session.preparing_mut()?.metadata.job =
        Some(client.submit_blocking(&scope, spec()?, |context| {
            Err(delivery::Failure::test_cause(&context, "Original retained phase failure"))
        })?);
    receiving.capture(&batch(&client, &wake)?)?;
    let original =
        std::ptr::from_ref(receiving.failure().ok_or("Actual phase cause missing")?).cast::<()>();
    for _ in 0..2 {
        assert!(
            receiving
                .receiving_session
                .retire_healthy_preparation(
                    inert_binding()?,
                    DiagnosticKey::new(
                        inert_binding()?.incident().subject().coordinator(),
                        DiagnosticRole::Reporter,
                        DiagnosticSessionId::new(inert_binding()?.session().get())?
                    )
                )
                .is_err()
        );
        assert!(receiving.receiving_session.recycled().is_none());
        assert!(receiving.receiving_session.retired().is_none());
        assert!(receiving.receiving_session.validate_disposal().is_err());
        assert_eq!(
            std::ptr::from_ref(receiving.failure().ok_or("Original phase cause lost")?)
                .cast::<()>(),
            original
        );
        assert!(receiving.receiving_session.preparing()?.metadata.completion.is_some());
    }
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 16 * 1024);
    assert_eq!(
        std::ptr::from_ref(receiving.failure().ok_or("Cause lost after join")?).cast::<()>(),
        original
    );
    drop(receiving);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
