// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Check actual finite delivery custody independently of native or durable acknowledgement proofs.
use super::*;
use std::{
    num::NonZeroUsize,
    sync::{Arc, Condvar, Mutex},
};
use sugarsubstitute_execution::ExecutionOwner;
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
    fn wake(&self) -> std::result::Result<(), ExecutionError> {
        *self.value.lock().unwrap_or_else(std::sync::PoisonError::into_inner) += 1;
        self.changed.notify_all();
        Ok(())
    }
}
fn spec() -> Result<JobSpec> {
    Ok(JobSpec {
        id: JobId::new(1)?,
        operation: Operation::NativeLifecycle,
        class: JobClass::InfrastructureLifecycle,
        deadline: Some(Moment::from_nanos(60_000_000_017)),
        reservation_bytes: 16 * 1024,
        replacement: None,
    })
}
fn batch(client: &ExecutionClient, wake: &Wake) -> Result<CompletionBatch> {
    loop {
        let before = *wake.value.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let actual = client.drain_completions(NonZeroUsize::MIN)?;
        if !actual.notifications.is_empty() {
            return Ok(actual);
        }
        let (value, timeout) = wake
            .changed
            .wait_timeout_while(
                wake.value.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
                Duration::from_secs(10),
                |value| *value == before,
            )
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if timeout.timed_out() && *value == before {
            return Err("Actual report receiving delivery stalled".into());
        }
    }
}
#[test]
fn incident_reporting_foreign_notice_cannot_take_actual_delivery_or_release_credit() -> Result<()> {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let original_spec = spec()?;
    let until = original_spec.deadline.ok_or("Missing test counter cutoff")?;
    let execution = client.clone();
    let mut delivery = delivery::Delivery::new();
    delivery.job = Some(client.submit_blocking(&scope, original_spec, move |context| {
        delivery::prepare_counter(&execution, until, &context)
    })?);
    let actual = batch(&client, &wake)?;
    let foreign_scope = ScopeId::new(2)?;
    let foreign = CompletionBatch {
        notifications: actual
            .notifications
            .iter()
            .map(|notice| CompletionNotification { scope_id: foreign_scope, ..*notice })
            .collect(),
        remaining: 0,
    };
    delivery.capture(&foreign)?;
    assert!(delivery.recyclable().is_err());
    assert!(delivery.job.is_some());
    assert!(delivery.completion.is_none());
    assert!(delivery.value()?.is_none());
    assert_eq!(scope.snapshot().reserved_bytes, 16 * 1024);
    delivery.capture(&actual)?;
    assert!(delivery.job.is_none());
    assert!(delivery.value()?.is_some());
    assert!(delivery.recyclable().is_ok());
    assert_eq!(delivery.completion.as_ref().map(|done| done.spec), Some(original_spec));
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 16 * 1024);
    drop(delivery);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn incident_reporting_failure_keeps_exact_native_cause_and_original_credit_after_join() -> Result<()>
{
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut delivery = delivery::Delivery::<()>::new();
    delivery.job = Some(client.submit_blocking(&scope, spec()?, move |context| {
        Err(delivery::Failure::test_cause(&context, "Original report producer rejection"))
    })?);
    delivery.capture(&batch(&client, &wake)?)?;
    assert!(delivery.value().is_err());
    assert!(delivery.recyclable().is_err());
    let actual = delivery
        .failure()
        .and_then(|cause| cause.source())
        .and_then(|cause| cause.downcast_ref::<std::io::Error>())
        .ok_or("Original report receiving native cause lost")?;
    assert_eq!(actual.to_string(), "Original report producer rejection");
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 16 * 1024);
    drop(delivery);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn incident_reporting_recycling_retains_actual_failed_preparation_and_idle_cause() -> Result<()> {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut receiving = IncidentReporting::new();
    receiving.receiving_session.preparing_mut()?.metadata.job =
        Some(client.submit_blocking(&scope, spec()?, |context| {
            Err(delivery::Failure::test_cause(&context, "Original recycle producer rejection"))
        })?);
    receiving.capture(&batch(&client, &wake)?)?;
    let actual = receiving.receiving_session.preparing()?;
    assert!(actual.metadata.recyclable().is_err());
    assert!(receiving.receiving_session.retired().is_none());
    let cause = receiving
        .failure()
        .and_then(std::error::Error::source)
        .and_then(|cause| cause.downcast_ref::<std::io::Error>())
        .ok_or("Original recycle cause lost")?;
    assert_eq!(cause.to_string(), "Original recycle producer rejection");
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 16 * 1024);
    assert!(receiving.receiving_session.preparing()?.metadata.recyclable().is_err());
    drop(receiving);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn incident_reporting_copied_retirement_flags_and_restart_action_cannot_reset_funded_source()
-> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
    crate::production::incident_host::tests::admitted_value(
        crate::production::incident_host::CONTROLLER_BYTES,
        |context| {
            let mut funding =
                Some(crate::production::reporter_role::parent::ReporterFunding::new(&context)?);
            let source = ReporterSource::new(&mut funding)?;
            assert_eq!(source.close(), Err(ReporterSourceIssue::Busy));
            let mut receiving = IncidentReporting::new();
            receiving.relay = Some(ReporterRelay::new(source));
            // Deliberately forged inert flags lack an actual sealed retirement receipt.
            receiving.family_retired = true;
            receiving.dismiss_action = Some(ReportAction::DismissAndRequestRestart);
            assert!(receiving.receiving_session.retired().is_none());
            receiving.finish_accepted_session().map_err(|_| ReporterSourceIssue::Parent)?;
            assert_eq!(receiving.dismiss_action, Some(ReportAction::DismissAndRequestRestart));
            assert!(!receiving.relay.as_ref().ok_or(ReporterSourceIssue::Busy)?.idle_funded());
            receiving.dismiss_action = None;
            assert!(receiving.finish_accepted_session().is_err());
            assert!(receiving.normal_close_budget().is_none());
            assert!(receiving.family_retired);
            assert!(!receiving.relay.as_ref().ok_or(ReporterSourceIssue::Busy)?.idle_funded());
            Ok(receiving.relay.take().ok_or(ReporterSourceIssue::Busy)?)
        },
    )
}
#[test]
fn incident_reporting_idle_guard_keeps_actual_metadata_until_matching_delivery() -> Result<()> {
    let wake = Arc::new(Wake::default());
    let execution = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = execution.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut receiving = IncidentReporting::new();
    assert!(!receiving.diagnostic_custody_pending());
    receiving.receiving_session.preparing_mut()?.metadata.job =
        Some(client.submit_blocking(&scope, spec()?, |context| {
            Err(delivery::Failure::test_cause(&context, "Original metadata boundary failure"))
        })?);
    assert!(receiving.diagnostic_custody_pending());
    let actual = batch(&client, &wake)?;
    let foreign_scope = ScopeId::new(2)?;
    let foreign = CompletionBatch {
        notifications: actual
            .notifications
            .iter()
            .map(|notice| CompletionNotification { scope_id: foreign_scope, ..*notice })
            .collect(),
        remaining: 0,
    };
    receiving.capture(&foreign)?;
    assert!(receiving.diagnostic_custody_pending());
    assert_eq!(scope.snapshot().reserved_bytes, 16 * 1024);
    receiving.capture(&actual)?;
    assert!(!receiving.diagnostic_custody_pending());
    assert!(receiving.failure().is_some());
    drop(execution);
    assert_eq!(scope.snapshot().reserved_bytes, 16 * 1024);
    drop(receiving);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[path = "tests/recycling_capture.rs"]
mod recycling_capture;

#[path = "tests/launch_input_storage.rs"]
mod launch_input_storage;

#[path = "tests/retirement_phase.rs"]
mod retirement_phase;

#[path = "tests/launch_jobs.rs"]
mod launch_jobs;
