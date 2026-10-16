// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Hold actual custodied native creation before ACK and retire unstarted inputs on stop.
use super::*;
use crate::{
    lifecycle_pipe::tests::{recovery_parent, recovery_server},
    native::NativeObservationError,
    process_creation::*,
    runtime_test_support::Gate,
};
use std::sync::atomic::{AtomicUsize, Ordering};
#[derive(Default)]
struct Effects {
    initialized: AtomicUsize,
    retired: AtomicUsize,
    dropped: AtomicUsize,
    ready: Gate,
}
struct Input(Arc<Effects>);
impl Drop for Input {
    fn drop(&mut self) {
        self.0.dropped.fetch_add(1, Ordering::AcqRel);
    }
}
struct Driver(Input);
impl ProcessCreationDriver for Driver {
    fn initialize(
        &mut self,
        _: &JobContext,
        memory: &ProcessCreationMemory,
        _: ProcessCreationWake,
    ) -> Result<(), NativeObservationError> {
        memory
            .check_effect()
            .map_err(|e| NativeObservationError::new("recovery_creation_test_admission", e))?;
        self.0.0.initialized.fetch_add(1, Ordering::AcqRel);
        self.0.0.ready.open();
        Ok(())
    }
    fn progress(
        &mut self,
        _: &JobContext,
        _: Moment,
    ) -> Result<ProcessCreationProgress, NativeObservationError> {
        Ok(ProcessCreationProgress::Started)
    }
    fn retire(&mut self, _: &JobContext, _: Moment) -> Result<bool, NativeObservationError> {
        self.0.0.retired.fetch_add(1, Ordering::AcqRel);
        Ok(true)
    }
}
fn spec_for(id: u64, grant: &RegisteredRecoveryGrant) -> TestResult<JobSpec> {
    let mut spec = spec(id, JobClass::InfrastructureLifecycle, PROCESS_CREATION_STORAGE_BYTES)?;
    spec.operation = Operation::NativeLifecycle;
    spec.deadline = Some(grant.original_budget().until());
    Ok(spec)
}
fn ack(key: RecoveryGrantKey) -> TestResult<DurableAcknowledgement> {
    Ok(DurableAcknowledgement::new(
        key.report().incident(),
        key.report().revision().next()?,
        IncidentUtc::from_unix_seconds(1)?,
    ))
}
/// Identify the original failed owner boundary without replacing its actual error source.
fn observed_stage<T>(stage: &'static str, result: TestResult<T>) -> TestResult<T> {
    result.map_err(|cause| Box::new(StageFailure { stage, cause }) as Box<dyn std::error::Error>)
}
#[derive(Debug)]
struct StageFailure {
    stage: &'static str,
    cause: Box<dyn std::error::Error>,
}
impl std::fmt::Display for StageFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Held recovery creation {}: {}", self.stage, self.cause)
    }
}
impl std::error::Error for StageFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.cause.as_ref())
    }
}
#[test]
fn recovery_creation_is_preheld_before_ack_and_runs_once_only_after_the_exact_persisted_decision()
-> TestResult {
    let rig = Rig::new()?;
    let boundary = Boundary::new(&rig, true)?;
    let (_scope, mut entry) = server(&rig, &boundary)?;
    let (source, memory) = registered(&rig, request_snapshot(&rig, &boundary, &entry)?)?;
    drop(memory);
    let mut completed = issued(&rig, &source)?;
    let snapshot = take(&mut completed)?;
    let grant =
        rig.client.issue_registered_recovery_grant(snapshot, &source, &completed, key()?)?;
    let (_custody_scope, mut custody, parent) = recovery_parent(&rig, 3, 4)?;
    let (_recovery_scope, mut recovery) = recovery_server(&rig, 4, 5, &grant)?;
    let effects = Arc::new(Effects::default());
    let mut input = Some(Input(effects.clone()));
    let permit = rig.client.reserve_recovery_custody_creation(
        &rig.scope,
        spec_for(6, &grant)?,
        &parent,
        &grant,
        &mut input,
        |input, _, _| Ok(Box::new(Driver(input))),
    )?;
    assert!(input.is_none());
    assert_eq!(effects.initialized.load(Ordering::Acquire), 0);
    // Independent real work proves the central worker can run while the creation is held.
    let mut unrelated = rig
        .client
        .submit_blocking(&rig.scope, spec_for(7, &grant)?, |_| Ok::<_, AdmissionError>(()))?;
    assert!(matches!(rig.completion(&mut unrelated)?.outcome, JobOutcome::Success(())));
    assert_eq!(effects.initialized.load(Ordering::Acquire), 0);
    assert_eq!(permit.key(), key()?);
    let current = ack(key()?)?;
    let changed =
        DurableAcknowledgement::new(current.incident(), current.revision().next()?, current.at());
    let rejected = permit.begin(changed).err().ok_or("Changed ACK activated held creation")?;
    assert_eq!(effects.initialized.load(Ordering::Acquire), 0);
    let mut job = rejected.permit.begin(ack(key()?)?).map_err(|failure| failure.error)?;
    effects.ready.wait()?;
    let actual = observed_stage("creation terminal", rig.completion(&mut job))?;
    assert!(matches!(actual.outcome, JobOutcome::Success(_)));
    assert_eq!(effects.initialized.load(Ordering::Acquire), 1);
    assert_eq!(effects.retired.load(Ordering::Acquire), 1);
    assert_eq!(effects.dropped.load(Ordering::Acquire), 1);
    let mut repeated = Some(Input(effects.clone()));
    assert!(
        rig.client
            .reserve_recovery_custody_creation(
                &rig.scope,
                spec_for(8, &grant)?,
                &parent,
                &grant,
                &mut repeated,
                |input, _, _| Ok(Box::new(Driver(input)))
            )
            .is_err()
    );
    assert!(repeated.is_some());
    drop(repeated);
    drop(actual);
    recovery.control.close();
    drop(observed_stage("recovery pipe retirement", rig.completion(&mut recovery.completion))?);
    custody.control.close();
    drop(observed_stage("custody pipe retirement", rig.completion(&mut custody.completion))?);
    drop(parent);
    drop(recovery);
    drop(custody);
    drop(grant);
    drop(completed);
    drop(source);
    close(&rig, &mut entry)
}
#[test]
fn recovery_creation_cancel_or_original_expiry_destroys_the_real_held_factory_without_effects()
-> TestResult {
    for expired in [false, true] {
        let rig = Rig::new()?;
        let boundary = Boundary::new(&rig, true)?;
        let (_scope, mut entry) = server(&rig, &boundary)?;
        let (source, memory) = registered(&rig, request_snapshot(&rig, &boundary, &entry)?)?;
        drop(memory);
        let mut completed = issued(&rig, &source)?;
        let snapshot = take(&mut completed)?;
        let grant =
            rig.client.issue_registered_recovery_grant(snapshot, &source, &completed, key()?)?;
        let (_custody_scope, mut custody, parent) = recovery_parent(&rig, 3, 4)?;
        let (_recovery_scope, mut recovery) = recovery_server(&rig, 4, 5, &grant)?;
        let effects = Arc::new(Effects::default());
        let mut input = Some(Input(effects.clone()));
        let permit = rig.client.reserve_recovery_custody_creation(
            &rig.scope,
            spec_for(6, &grant)?,
            &parent,
            &grant,
            &mut input,
            |input, _, _| Ok(Box::new(Driver(input))),
        )?;
        if expired {
            rig.clock.advance(grant.original_budget().until().as_nanos());
        } else {
            permit.request_cancel();
        }
        let mut job = permit.cancel_and_observe();
        let actual = observed_stage("creation terminal", rig.completion(&mut job))?;
        assert!(if expired {
            matches!(actual.outcome, JobOutcome::TimedOut)
        } else {
            matches!(actual.outcome, JobOutcome::Cancelled)
        });
        assert_eq!(effects.initialized.load(Ordering::Acquire), 0);
        assert_eq!(effects.retired.load(Ordering::Acquire), 0);
        assert_eq!(effects.dropped.load(Ordering::Acquire), 1);
        drop(actual);
        recovery.control.close();
        drop(observed_stage("recovery pipe retirement", rig.completion(&mut recovery.completion))?);
        custody.control.close();
        drop(observed_stage("custody pipe retirement", rig.completion(&mut custody.completion))?);
        drop(parent);
        drop(recovery);
        drop(custody);
        drop(grant);
        drop(completed);
        drop(source);
        observed_stage("Root Entry retirement", close(&rig, &mut entry))?;
    }
    Ok(())
}

#[path = "recovery_creation_tests/admission.rs"]
mod admission;
