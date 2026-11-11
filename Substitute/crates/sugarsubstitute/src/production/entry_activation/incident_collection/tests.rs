// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Verify original failed facts and finite metadata custody without creating capture/store proof.
use super::*;
use crate::completion_wake::Wake;
use std::sync::Arc;
use sugarsubstitute_contracts::{execution::*, lifecycle::*};
use sugarsubstitute_execution::ExecutionOwner;
type TestError = Box<dyn Error + Send + Sync>;
/// A deterministic execution clock; wallclock sampling is separately injected into preparation.
struct TestClock;
impl Clock for TestClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(17)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
/// Mint sealed data through the authoritative exit transaction; this claims no native observation.
fn fact(status: i64) -> Result<FailedDesktopFact> {
    let owner = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    };
    let run = RunKey { owner, instance: InstanceId::new(3)?, generation: RunGeneration::new(4)? };
    let process = ProcessKey { owner, id: ProcessId::new(5)? };
    let mut exit =
        sugarsubstitute_application::supervision::DesktopExitTransaction::new(run, process)?;
    exit.observe(DesktopObservation::Exited { process, code: status })
        .map_err(|rejected| rejected.reason)?;
    Ok(exit.failed_incident()?)
}
#[test]
fn incident_collection_keeps_sealed_zero_exit_and_first_original_cutoff() -> Result<()> {
    let actual = fact(0)?;
    assert_eq!(actual.attribution(), DesktopExitKind::Unexpected);
    assert_eq!(actual.actual_status(), 0);
    let mut collection = IncidentCollection::new();
    collection.observe(actual, Moment::from_nanos(17))?;
    let original = collection.original.ok_or("original budget absent")?;
    collection.observe(actual, Moment::from_nanos(18_000_000_000))?;
    assert_eq!(collection.original, Some(original));
    assert!(collection.observe(fact(9)?, Moment::from_nanos(19_000_000_000)).is_err());
    assert_eq!(collection.fact, Some(actual));
    assert!(collection.pending());
    assert!(collection.request.is_none());
    assert!(collection.capture.is_none());
    assert!(collection.scope.is_none());
    Ok(())
}
#[test]
fn incident_collection_close_gate_keeps_preclose_fact_until_adoption_and_exposes_first_cutoff()
-> Result<()> {
    let mut collection = IncidentCollection::new();
    assert!(!collection.pending());
    assert!(collection.delivered());
    assert!(collection.next_deadline().is_none());
    collection.observe(fact(0)?, Moment::from_nanos(17))?;
    let cutoff = collection.original.ok_or("original interval absent")?.until();
    assert!(collection.pending());
    assert_eq!(collection.next_deadline(), Some(cutoff));
    collection.observe(fact(0)?, Moment::from_nanos(200))?;
    assert_eq!(collection.next_deadline(), Some(cutoff));
    collection.first = Some(Box::new(std::io::Error::other("failure before native admission")));
    assert!(!collection.pending());
    assert!(collection.next_deadline().is_none());
    Ok(())
}
#[test]
fn incident_collection_failed_installed_control_waits_for_checked_disposition() -> Result<()> {
    let mut collection = IncidentCollection::new();
    collection.observe(fact(0)?, Moment::from_nanos(17))?;
    collection.installed = true;
    collection.first = Some(Box::new(std::io::Error::other("original collection failure")));
    let original = collection.failure().map(std::ptr::from_ref);
    assert!(collection.pending());
    assert!(collection.next_deadline().is_some());
    // Exercise only the caller gate; this sets no repository/native retirement proof.
    collection.checked_cleanup = true;
    assert!(!collection.pending());
    assert!(collection.next_deadline().is_none());
    assert!(!collection.adopted);
    assert_eq!(collection.failure().map(std::ptr::from_ref), original);
    assert!(collection.scope.is_none());
    assert!(collection.capture.is_none());
    Ok(())
}
#[test]
fn incident_collection_real_preparation_keeps_utc_fact_and_original_charge_after_worker_join()
-> std::result::Result<(), TestError> {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let actual = fact(-9).map_err(|cause| std::io::Error::other(cause.to_string()))?;
    let original = IncidentOperationBudget::new(Moment::from_nanos(17))?;
    let mut job = client.submit_blocking(
        &scope,
        JobSpec {
            id: JobId::new(1)?,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(original.until()),
            reservation_bytes: 32768,
            replacement: None,
        },
        move |context| {
            material::prepare(actual, original, &context, || {
                Ok(IncidentUtc::from_unix_seconds(110)?)
            })
        },
    )?;
    let done = wake.completion(&client, &mut job).map_err(|cause| cause.to_string())?;
    let material = match done.outcome {
        JobOutcome::Success(material) => material,
        JobOutcome::Failure(cause) => return Err(cause.into()),
        _ => return Err("actual metadata worker did not succeed".into()),
    };
    assert_eq!(material.fact, actual);
    assert_eq!(material.context.budget(), original);
    assert_eq!(material.context.at(), IncidentUtc::from_unix_seconds(110)?);
    drop(done.memory);
    drop(job);
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 32768);
    drop(material);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn incident_collection_real_preparation_preserves_original_wallclock_error_and_full_credit()
-> std::result::Result<(), TestError> {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(TestClock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let actual = fact(0).map_err(|cause| std::io::Error::other(cause.to_string()))?;
    let original = IncidentOperationBudget::new(Moment::from_nanos(17))?;
    let mut job = client.submit_blocking(
        &scope,
        JobSpec {
            id: JobId::new(1)?,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(original.until()),
            reservation_bytes: 32768,
            replacement: None,
        },
        move |context| {
            material::prepare(actual, original, &context, || {
                Err(Box::new(std::io::Error::other("injected external UTC rejection")))
            })
        },
    )?;
    let done = wake.completion(&client, &mut job).map_err(|cause| cause.to_string())?;
    let failure = match done.outcome {
        JobOutcome::Failure(cause) => cause,
        _ => return Err("unexpected failure metadata result".into()),
    };
    assert!(failure.source().and_then(|cause| cause.downcast_ref::<std::io::Error>()).is_some());
    drop(done.memory);
    drop(job);
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 32768);
    drop(failure);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
