// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe actual boxed-input rejection, source failure and original storage teardown.
use crate::{
    BlockingInputError, ChargedBlockingInput,
    admission::Core,
    runtime_test_support::{Gate, Rig, TestResult, spec},
};
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};
use sugarsubstitute_contracts::execution::*;
struct Release(Arc<Gate>);
impl Drop for Release {
    fn drop(&mut self) {
        self.0.open();
    }
}
#[derive(Debug)]
struct Input {
    payload: [u8; 8192],
    identity: Arc<()>,
    memory: Option<MemoryLease>,
    dropped: Arc<AtomicUsize>,
    funded_at_drop: Arc<AtomicBool>,
}
impl Input {
    fn new() -> Self {
        Self {
            payload: [17; 8192],
            identity: Arc::new(()),
            memory: None,
            dropped: Arc::new(AtomicUsize::new(0)),
            funded_at_drop: Arc::new(AtomicBool::new(false)),
        }
    }
}
impl Drop for Input {
    fn drop(&mut self) {
        if let Some(memory) = &self.memory {
            self.funded_at_drop.store(memory.claim(memory.bytes()).is_err(), Ordering::Release);
        }
        self.dropped.fetch_add(1, Ordering::AcqRel);
    }
}
struct RejectedFactoryDrop {
    core: Arc<Core>,
    scope: ScopeId,
    observed: Arc<AtomicBool>,
}
impl Drop for RejectedFactoryDrop {
    fn drop(&mut self) {
        self.observed
            .store(self.core.scope_snapshot(self.scope).reserved_bytes == 0, Ordering::Release);
    }
}
#[derive(Debug)]
struct SourceFailure {
    input: ChargedBlockingInput<Input>,
    boundary: Result<(), io::Error>,
}
#[test]
fn actual_input_claim_rejection_preserves_source_and_disposes_factory_after_core_unlock()
-> TestResult {
    let rig = Rig::new()?;
    let value = Input::new();
    let identity = value.identity.clone();
    let dropped = value.dropped.clone();
    let mut input = Some(value);
    let observed = Arc::new(AtomicBool::new(false));
    let factory_drop = RejectedFactoryDrop {
        core: rig.client.core_for_test(),
        scope: rig.scope.id(),
        observed: observed.clone(),
    };
    let calls = Arc::new(AtomicUsize::new(0));
    let called = calls.clone();
    let rejection = rig.client.submit_blocking_owned_box(
        &rig.scope,
        spec(1, JobClass::InfrastructureOrdinary, 4096)?,
        &mut input,
        move |_, _| {
            drop(factory_drop);
            called.fetch_add(1, Ordering::AcqRel);
            Ok::<_, io::Error>(())
        },
    );
    let Err(BlockingInputError::Admission(AdmissionError::Memory(cause))) = rejection else {
        return Err("Actual constructor claim cause absent".into());
    };
    assert_eq!(cause.requested_bytes, size_of::<Input>() as u64);
    assert_eq!(cause.available_bytes, 4096);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    assert!(observed.load(Ordering::Acquire));
    assert_eq!(calls.load(Ordering::Acquire), 0);
    assert_eq!(dropped.load(Ordering::Acquire), 0);
    let returned = input.as_ref().ok_or("Original input lost")?;
    assert!(Arc::ptr_eq(&returned.identity, &identity));
    assert_eq!(returned.payload, [17; 8192]);
    let mut job = rig.client.submit_blocking_owned_box(
        &rig.scope,
        spec(1, JobClass::InfrastructureOrdinary, 4194304)?,
        &mut input,
        |value, _| Ok::<_, io::Error>(value),
    )?;
    assert!(input.is_none());
    let completion = rig.completion(&mut job)?;
    let JobOutcome::Success(value) = completion.outcome else {
        return Err("Rejection consumed original job identity".into());
    };
    assert!(Arc::ptr_eq(&value.input().identity, &identity));
    drop(value);
    assert_eq!(dropped.load(Ordering::Acquire), 1);
    Ok(())
}
#[test]
fn failed_charged_box_survives_actual_cancel_ack_and_join_until_original_data_drop() -> TestResult {
    let rig = Rig::new()?;
    let entered = Arc::new(Gate::default());
    let returned = Arc::new(Gate::default());
    let _release = Release(returned.clone());
    let value = Input::new();
    let identity = value.identity.clone();
    let dropped = value.dropped.clone();
    let funded = value.funded_at_drop.clone();
    let mut input = Some(value);
    let worker_entered = entered.clone();
    let worker_returned = returned.clone();
    let mut job = rig.client.submit_blocking_owned_box(
        &rig.scope,
        spec(1, JobClass::InfrastructureOrdinary, 4194304)?,
        &mut input,
        move |mut input, context| {
            input.input_mut().memory = Some(context.memory().clone());
            let mut cause = SourceFailure { input, boundary: Ok(()) };
            worker_entered.open();
            cause.boundary =
                worker_returned.wait().map_err(|error| io::Error::other(error.to_string()));
            Err::<(), _>(cause)
        },
    )?;
    entered.wait()?;
    job.cancel();
    returned.open();
    let completion = rig.completion(&mut job)?;
    let JobOutcome::Failure(cause) = completion.outcome else {
        return Err("Actual earlier box source replaced by stop".into());
    };
    assert!(cause.boundary.is_ok());
    assert!(Arc::ptr_eq(&cause.input.input().identity, &identity));
    drop(completion.memory);
    drop(rig.owner);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 4194304);
    assert_eq!(dropped.load(Ordering::Acquire), 0);
    drop(cause);
    assert!(funded.load(Ordering::Acquire));
    assert_eq!(dropped.load(Ordering::Acquire), 1);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn cancelled_boxed_success_destroys_funded_data_before_worker_join_returns_credit() -> TestResult {
    let rig = Rig::new()?;
    let entered = Arc::new(Gate::default());
    let returned = Arc::new(Gate::default());
    let _release = Release(returned.clone());
    let value = Input::new();
    let dropped = value.dropped.clone();
    let funded = value.funded_at_drop.clone();
    let mut input = Some(value);
    let worker_entered = entered.clone();
    let worker_returned = returned.clone();
    let mut job = rig.client.submit_blocking_owned_box(
        &rig.scope,
        spec(1, JobClass::InfrastructureOrdinary, 4194304)?,
        &mut input,
        move |mut input, context| {
            input.input_mut().memory = Some(context.memory().clone());
            worker_entered.open();
            worker_returned.wait().map_err(|error| io::Error::other(error.to_string()))?;
            Ok::<_, io::Error>(input)
        },
    )?;
    entered.wait()?;
    job.cancel();
    returned.open();
    let completion = rig.completion(&mut job)?;
    assert!(matches!(completion.outcome, JobOutcome::Cancelled));
    drop(completion);
    drop(rig.owner);
    assert!(funded.load(Ordering::Acquire));
    assert_eq!(dropped.load(Ordering::Acquire), 1);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}
