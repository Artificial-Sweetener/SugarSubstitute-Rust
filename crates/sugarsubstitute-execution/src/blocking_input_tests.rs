// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Prove real rejection custody and source-first finite worker outcomes.
use crate::{
    OwnedInputError,
    runtime_test_support::{Gate, Rig, TestResult, spec},
};
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use sugarsubstitute_contracts::execution::*;
/// Opens the external return boundary before the real worker joins during teardown.
struct Release(Arc<Gate>);
impl Drop for Release {
    fn drop(&mut self) {
        self.0.open();
    }
}
#[derive(Debug)]
struct SourceFailure {
    identity: Arc<()>,
    _claim: Result<MemoryClaim, MemoryClaimError>,
    _boundary: Result<(), io::Error>,
}

#[test]
fn owned_input_rejection_returns_identical_input_without_factory_or_memory_effect() -> TestResult {
    let rig = Rig::new()?;
    let original = Arc::new(());
    let mut input = Some(original.clone());
    let calls = Arc::new(AtomicUsize::new(0));
    let worker = calls.clone();
    let rejected = rig.client.submit_blocking_owned_input(
        &rig.scope,
        spec(1, JobClass::IoLifecycle, 4096)?,
        &mut input,
        move |_, _| {
            worker.fetch_add(1, Ordering::AcqRel);
            Ok::<_, io::Error>(())
        },
    );
    assert!(matches!(rejected, Err(OwnedInputError::Admission(AdmissionError::WrongClass { .. }))));
    assert!(Arc::ptr_eq(input.as_ref().ok_or("Original input lost")?, &original));
    assert_eq!(calls.load(Ordering::Acquire), 0);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    let mut missing: Option<Arc<()>> = None;
    assert!(matches!(
        rig.client.submit_blocking_owned_input(
            &rig.scope,
            spec(1, JobClass::InfrastructureOrdinary, 4096)?,
            &mut missing,
            |_, _| Ok::<_, io::Error>(())
        ),
        Err(OwnedInputError::MissingInput)
    ));
    let mut job = rig.client.submit_blocking_owned_input(
        &rig.scope,
        spec(1, JobClass::InfrastructureOrdinary, 4096)?,
        &mut input,
        |value, _| Ok::<_, io::Error>(value),
    )?;
    assert!(input.is_none());
    let completion = rig.completion(&mut job)?;
    let JobOutcome::Success(value) = completion.outcome else {
        return Err("Actual owned factory failed".into());
    };
    assert!(Arc::ptr_eq(&value, &original));
    Ok(())
}

#[test]
fn earlier_owned_source_and_original_claim_survive_late_actual_job_cancellation() -> TestResult {
    let rig = Rig::new()?;
    let returned = Arc::new(Gate::default());
    let _release = Release(returned.clone());
    let entered = Arc::new(Gate::default());
    let source = Arc::new(());
    let mut input = Some(source.clone());
    let worker_entered = entered.clone();
    let worker_returned = returned.clone();
    let mut job = rig.client.submit_blocking_owned_input(
        &rig.scope,
        spec(1, JobClass::InfrastructureOrdinary, 4194304)?,
        &mut input,
        move |value, context| {
            let mut cause = SourceFailure {
                identity: value,
                _claim: context.memory().claim(2097152),
                _boundary: Ok(()),
            };
            worker_entered.open();
            cause._boundary =
                worker_returned.wait().map_err(|error| io::Error::other(error.to_string()));
            Err::<(), _>(cause)
        },
    )?;
    entered.wait()?;
    job.cancel();
    returned.open();
    let completion = rig.completion(&mut job)?;
    let JobOutcome::Failure(cause) = completion.outcome else {
        return Err("Original source was replaced".into());
    };
    assert!(Arc::ptr_eq(&cause.identity, &source));
    assert!(cause._claim.is_ok());
    assert!(cause._boundary.is_ok());
    assert_eq!(rig.scope.snapshot().reserved_bytes, 4194304);
    drop(completion.memory);
    drop(rig.owner);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 4194304);
    drop(cause);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn owned_success_obeys_cancel_veto_and_ordinary_failure_keeps_existing_stop_precedence()
-> TestResult {
    let rig = Rig::new()?;
    let returned = Arc::new(Gate::default());
    let _release = Release(returned.clone());
    let entered = Arc::new(Gate::default());
    let worker_entered = entered.clone();
    let worker_returned = returned.clone();
    let mut input = Some(());
    let mut job = rig.client.submit_blocking_owned_input(
        &rig.scope,
        spec(1, JobClass::InfrastructureOrdinary, 4096)?,
        &mut input,
        move |_, _| {
            worker_entered.open();
            worker_returned.wait().map_err(|cause| io::Error::other(cause.to_string()))?;
            Ok::<_, io::Error>(())
        },
    )?;
    entered.wait()?;
    job.cancel();
    returned.open();
    assert!(matches!(rig.completion(&mut job)?.outcome, JobOutcome::Cancelled));
    let returned = Arc::new(Gate::default());
    let _release = Release(returned.clone());
    let entered = Arc::new(Gate::default());
    let worker_entered = entered.clone();
    let worker_returned = returned.clone();
    let mut job = rig.client.submit_blocking(
        &rig.scope,
        spec(2, JobClass::InfrastructureOrdinary, 4096)?,
        move |_| {
            let cause = io::Error::other("Original finite failure");
            worker_entered.open();
            worker_returned.wait().map_err(|cause| io::Error::other(cause.to_string()))?;
            Err::<(), _>(cause)
        },
    )?;
    entered.wait()?;
    job.cancel();
    returned.open();
    assert!(matches!(rig.completion(&mut job)?.outcome, JobOutcome::Cancelled));
    Ok(())
}
