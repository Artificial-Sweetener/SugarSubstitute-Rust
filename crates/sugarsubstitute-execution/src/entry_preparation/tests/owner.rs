// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drive native effects through genuine admission, original controls and result custody.

use super::super::{
    CapturedEntryEnvironment, EntryEnvironmentFailure, EntryPreparationCause,
    EntryPreparationFailure, EntryPreparationSuccess,
    cause::EntryEnvironmentCause,
    control::EntryPreparationControlCause,
    core_binding::PREPARATION_BYTES,
    port::{AllocationState, EnvironmentPort},
};
use super::support::{Behavior, Calls, port};
use crate::{
    runtime_test_support::{Rig, TestResult, spec},
    scope::lock,
};
use std::{
    error::Error,
    num::NonZeroUsize,
    sync::{Arc, Mutex},
};
use sugarsubstitute_contracts::{coordinator_entry::EntrySetupBudget, execution::*};

struct CallbackPort {
    inner: Box<dyn EnvironmentPort>,
    after_get: Option<Box<dyn FnOnce() + Send>>,
}
impl EnvironmentPort for CallbackPort {
    fn get(&mut self) -> Result<(), EntryEnvironmentCause> {
        let outcome = self.inner.get();
        if let Some(callback) = self.after_get.take() {
            callback();
        }
        outcome
    }
    fn read_unit(&mut self, index: usize) -> Result<u16, EntryEnvironmentCause> {
        self.inner.read_unit(index)
    }
    fn free(&mut self) -> Result<(), EntryEnvironmentCause> {
        self.inner.free()
    }
    fn allocation_state(&self) -> AllocationState {
        self.inner.allocation_state()
    }
}
type Completion = JobCompletion<
    EntryPreparationSuccess<CapturedEntryEnvironment>,
    EntryPreparationFailure<EntryEnvironmentFailure>,
>;
fn admitted_capture(
    rig: &Rig,
    behavior: Behavior,
    after_get: Option<Box<dyn FnOnce() + Send>>,
) -> TestResult<(Completion, Arc<Mutex<Calls>>)> {
    let (mut job, calls) = capture_job(rig, behavior, after_get)?;
    Ok((rig.completion(&mut job)?, calls))
}
type CaptureJob = OwnedJob<
    EntryPreparationSuccess<CapturedEntryEnvironment>,
    EntryPreparationFailure<EntryEnvironmentFailure>,
>;
fn capture_job(
    rig: &Rig,
    behavior: Behavior,
    after_get: Option<Box<dyn FnOnce() + Send>>,
) -> TestResult<(CaptureJob, Arc<Mutex<Calls>>)> {
    let (inner, calls) = port(vec![65, 61, 0xd800, 0, 0], behavior);
    let boundary = CallbackPort { inner, after_get };
    let setup = EntrySetupBudget::new(Moment::from_nanos(0))?;
    let mut request = spec(1, JobClass::InfrastructureLifecycle, PREPARATION_BYTES)?;
    request.operation = Operation::NativeLifecycle;
    request.deadline = Some(setup.until());
    let job = rig
        .client
        .submit_entry_preparation(&rig.scope, request, setup, move |context| {
            context.capture_with_port(Box::new(boundary))
        })
        .map_err(|rejected| rejected.error)?;
    Ok((job, calls))
}

#[test]
fn genuine_native_copy_keeps_original_whole_claim_after_terminal_ack() -> TestResult {
    let rig = Rig::new()?;
    let (completion, calls) = admitted_capture(&rig, Behavior::default(), None)?;
    let JobOutcome::Success(success) = completion.outcome else {
        return Err("Actual synthetic capture did not deliver checked units".into());
    };
    drop(completion.memory);
    assert_eq!(success.value().units(), &[65, 61, 0xd800, 0, 0]);
    assert_eq!(lock(&calls).get, 1);
    assert_eq!(lock(&calls).free, 1);
    assert_eq!(rig.scope.snapshot().reserved_bytes, PREPARATION_BYTES);
    assert!(!lock(&rig.client.core.state).jobs.contains_key(&JobId::new(1)?));
    drop(success);
    let Rig { owner, scope, .. } = rig;
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn original_cancel_during_get_still_permits_first_same_block_free() -> TestResult {
    let rig = Rig::new()?;
    let core = rig.client.core.clone();
    let id = rig.scope.id();
    let (completion, calls) =
        admitted_capture(&rig, Behavior::default(), Some(Box::new(move || core.cancel_scope(id))))?;
    let JobOutcome::Failure(failure) = completion.outcome else {
        return Err("Original cancellation published copied data".into());
    };
    let EntryPreparationCause::Factory(environment) = failure.cause() else {
        return Err("Later original cancellation replaced the capture's first cause".into());
    };
    assert!(matches!(
        environment.cause(),
        Some(EntryEnvironmentCause::Control { source: EntryPreparationControlCause::Cancelled })
    ));
    assert_eq!(
        environment.observation().ok_or("Actual observation missing")?.phase,
        super::super::state::EntryEnvironmentPhase::Released
    );
    assert_eq!(lock(&calls).free, 1);
    assert_eq!(lock(&calls).reads, 0);
    assert_eq!(rig.scope.snapshot().reserved_bytes, PREPARATION_BYTES);
    Ok(())
}

#[test]
fn original_expiry_during_get_never_renews_cleanup_budget() -> TestResult {
    let rig = Rig::new()?;
    let clock = rig.clock.clone();
    let (completion, calls) = admitted_capture(
        &rig,
        Behavior::default(),
        Some(Box::new(move || clock.advance(EntrySetupBudget::DURATION.as_secs() * 1_000_000_000))),
    )?;
    let JobOutcome::Failure(failure) = completion.outcome else {
        return Err("Expired original preparation published copied data".into());
    };
    let EntryPreparationCause::Factory(environment) = failure.cause() else {
        return Err("Original expired capture source was replaced".into());
    };
    assert!(matches!(
        environment.cause(),
        Some(EntryEnvironmentCause::Control { source: EntryPreparationControlCause::Deadline })
    ));
    assert_eq!(lock(&calls).free, 1);
    assert_eq!(lock(&calls).reads, 0);
    assert_eq!(rig.scope.snapshot().reserved_bytes, PREPARATION_BYTES);
    Ok(())
}

#[test]
fn actual_provider_panic_after_get_keeps_primary_and_checked_first_free() -> TestResult {
    let rig = Rig::new()?;
    let clock = rig.clock.clone();
    let (mut job, calls) = capture_job(
        &rig,
        Behavior::default(),
        Some(Box::new(move || {
            clock.on_next_current_thread_observation(Box::new(|| {
                std::panic::resume_unwind(Box::new("synthetic actual original provider panic"))
            }))
        })),
    )?;
    let mut mailbox_failure = None;
    let completion = loop {
        let observed = rig.wake.count();
        if let Err(cause) = rig.client.drain_completions(NonZeroUsize::MIN)
            && mailbox_failure.replace(cause).is_some()
        {
            return Err("Actual fatal mailbox failure was emitted twice".into());
        }
        if let Some(completion) = job.try_take_completion() {
            break completion;
        }
        rig.wake.wait_after(observed)?;
    };
    assert!(mailbox_failure.ok_or("Actual fatal mailbox failure was missing")?.source().is_some());
    let JobOutcome::Failure(failure) = completion.outcome else {
        return Err("Actual original provider panic published copied data".into());
    };
    let EntryPreparationCause::Factory(environment) = failure.cause() else {
        return Err("Later post-provider replaced first original panic".into());
    };
    assert!(matches!(
        environment.cause(),
        Some(EntryEnvironmentCause::Control {
            source: EntryPreparationControlCause::Unexpected(_)
        })
    ));
    assert_eq!(lock(&calls).free, 1);
    assert_eq!(lock(&calls).reads, 0);
    assert_eq!(rig.scope.snapshot().reserved_bytes, PREPARATION_BYTES);
    Ok(())
}
