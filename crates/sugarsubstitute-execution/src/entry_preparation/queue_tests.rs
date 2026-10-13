// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Test irreversible custody with actual admission and selectors before starting workers.

use super::{
    EntryPreparationContext, EntryPreparationFailure, EntryPreparationInputError,
    EntryPreparationSuccess,
    core_binding::{OriginalPreparation, PREPARATION_BYTES},
    owner::EntryPreparationOwner,
    port::{AllocationState, EnvironmentPort},
};
use crate::{
    admission::{Core, Retirement},
    runtime_test_support::{ClockPort, ManualClock, TestResult, Wake, spec},
    scope::lock,
    task::{QueuedJob, Stop},
};
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use sugarsubstitute_contracts::{coordinator_entry::EntrySetupBudget, execution::*};

/// The genuine adapter admits and dequeues this job; no native worker is started.
/// Permanently uncertain synthetic Free therefore cannot hang a worker's final join.
struct Fixture {
    core: Arc<Core>,
    scope: Scope,
    queued: QueuedJob,
    job: Option<OwnedJob<EntryPreparationSuccess<()>, EntryPreparationFailure<io::Error>>>,
    owner: EntryPreparationOwner,
}
impl Fixture {
    fn new() -> TestResult<Self> {
        let manual = Arc::new(ManualClock::default());
        let core =
            Core::new(ExecutionId::new(1)?, Arc::new(ClockPort(manual)), Arc::new(Wake::default()));
        let client = crate::ExecutionClient { core: core.clone() };
        let scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        let setup = EntrySetupBudget::new(Moment::from_nanos(0))?;
        let mut specification = spec(1, JobClass::InfrastructureLifecycle, PREPARATION_BYTES)?;
        specification.operation = Operation::NativeLifecycle;
        specification.deadline = Some(setup.until());
        let job = client
            .submit_entry_preparation(&scope, specification, setup, |_| Ok::<_, io::Error>(()))
            .map_err(|rejected| rejected.error)?;
        let queued = core
            .take(|class| class == JobClass::InfrastructureLifecycle)
            .ok_or("Actual preparation selector did not dequeue its admitted job")?;
        let (binding, memory) = OriginalPreparation::mint(&core, &queued.context, setup)?;
        let owner = EntryPreparationOwner::new(core.clone(), binding, memory).map_err(
            |(cause, memory)| {
                drop(memory);
                cause
            },
        )?;
        owner.register()?;
        Ok(Self { core, scope, queued, job: Some(job), owner })
    }
    fn handoff(&self) -> TestResult {
        self.core.handoff_entry_preparation(self.owner.clone()).map_err(|(cause, returned)| {
            drop(returned);
            cause
        })?;
        Ok(())
    }
    fn stop(&self) {
        self.core.cancel_job(self.queued.context.job_id());
        self.core.flush_notifications();
        self.queued.job.stop(&self.queued.context, Some(Moment::from_nanos(0)), Stop::Cancelled);
    }
    fn assert_one_native_entry(&self) {
        let state = lock(&self.core.state);
        assert_eq!(state.retired.len(), 1);
        assert!(matches!(state.retired.front(), Some(Retirement::Environment(_))));
        drop(state);
        assert_eq!(self.scope.snapshot().reserved_bytes, PREPARATION_BYTES);
    }
}

struct Port {
    state: AllocationState,
    frees: Arc<AtomicUsize>,
    fail_free: bool,
}
impl EnvironmentPort for Port {
    fn get(&mut self) -> Result<(), super::cause::EntryEnvironmentCause> {
        self.state = AllocationState::Held;
        Ok(())
    }
    fn read_unit(&mut self, index: usize) -> Result<u16, super::cause::EntryEnvironmentCause> {
        if index < 2 { Ok(0) } else { Err(super::cause::EntryEnvironmentCause::InvalidNativeState) }
    }
    fn free(&mut self) -> Result<(), super::cause::EntryEnvironmentCause> {
        self.frees.fetch_add(1, Ordering::AcqRel);
        self.state = AllocationState::RetirementUncertain;
        if self.fail_free {
            return Err(super::cause::EntryEnvironmentCause::InvalidNativeState);
        }
        self.state = AllocationState::Released;
        Ok(())
    }
    fn allocation_state(&self) -> AllocationState {
        self.state
    }
}
fn port(frees: &Arc<AtomicUsize>, fail_free: bool) -> Box<dyn EnvironmentPort> {
    Box::new(Port { state: AllocationState::Vacant, frees: frees.clone(), fail_free })
}

#[test]
fn abandoned_terminal_coalesces_into_one_actual_uncertain_native_handoff() -> TestResult {
    let mut fixture = Fixture::new()?;
    let frees = Arc::new(AtomicUsize::new(0));
    let context = EntryPreparationContext::new(fixture.owner.clone());
    let failure = context
        .capture_with_port(port(&frees, true))
        .err()
        .ok_or("Actual failed Free was reported as released")?;
    assert_eq!(frees.load(Ordering::Acquire), 1);
    fixture.handoff()?;
    let (cause, returned) = fixture
        .core
        .handoff_entry_preparation(fixture.owner.clone())
        .err()
        .ok_or("Duplicate original native handoff was accepted")?;
    assert!(matches!(cause, EntryPreparationInputError::Closed));
    assert!(returned.same_owner(&fixture.owner));
    drop(returned);
    drop(fixture.job.take());
    fixture.stop();
    fixture.assert_one_native_entry();
    assert!(lock(&fixture.owner.passive_slot()?.state).pending_result_disposal);
    assert!(fixture.core.take_retired_cpu(|_| true).is_none());
    let mut retirement = fixture
        .core
        .take_environment_retirement()
        .ok_or("Actual native selector lost held preparation")?;
    retirement.progress(&fixture.core);
    retirement.progress(&fixture.core);
    assert!(!retirement.completed());
    assert_eq!(frees.load(Ordering::Acquire), 1);
    assert_eq!(fixture.scope.snapshot().reserved_bytes, PREPARATION_BYTES);
    assert!(lock(&fixture.owner.passive_slot()?.state).native_retirement_active);
    drop(failure);
    Ok(())
}

#[test]
fn acknowledgement_keeps_actual_uncertain_custody_without_duplicate_delivery() -> TestResult {
    let mut fixture = Fixture::new()?;
    let frees = Arc::new(AtomicUsize::new(0));
    let context = EntryPreparationContext::new(fixture.owner.clone());
    let failure = context
        .capture_with_port(port(&frees, true))
        .err()
        .ok_or("Actual failed Free lost custody")?;
    fixture.handoff()?;
    fixture.stop();
    let completion = fixture
        .job
        .as_mut()
        .and_then(OwnedJob::try_take_completion)
        .ok_or("Actual typed terminal completion was absent")?;
    assert!(matches!(completion.outcome, JobOutcome::Cancelled));
    drop(completion);
    assert!(
        lock(&fixture.core.state)
            .jobs
            .get(&fixture.queued.context.job_id())
            .is_some_and(|record| record.acknowledged && record.held_native())
    );
    fixture.assert_one_native_entry();
    assert!(fixture.job.as_mut().and_then(OwnedJob::try_take_completion).is_none());
    let mut retirement = fixture
        .core
        .take_environment_retirement()
        .ok_or("Acknowledgement discarded actual held native entry")?;
    retirement.progress(&fixture.core);
    assert!(!retirement.completed());
    assert_eq!(frees.load(Ordering::Acquire), 1);
    assert_eq!(fixture.scope.snapshot().reserved_bytes, PREPARATION_BYTES);
    drop(failure);
    Ok(())
}

#[test]
fn actual_settled_native_entry_disposes_abandoned_result_once_and_releases_original_memory()
-> TestResult {
    let mut fixture = Fixture::new()?;
    let frees = Arc::new(AtomicUsize::new(0));
    let context = EntryPreparationContext::new(fixture.owner.clone());
    let environment = context.capture_with_port(port(&frees, false))?;
    fixture.handoff()?;
    drop(fixture.job.take());
    fixture.stop();
    fixture.assert_one_native_entry();
    let mut retirement = fixture
        .core
        .take_environment_retirement()
        .ok_or("Actual settled native entry was absent")?;
    retirement.progress(&fixture.core);
    retirement.progress(&fixture.core);
    assert!(retirement.completed());
    assert_eq!(frees.load(Ordering::Acquire), 1);
    assert!(lock(&fixture.core.state).jobs.is_empty());
    assert!(lock(&fixture.core.state).retired.is_empty());
    let core = fixture.core.clone();
    let scope = fixture.scope;
    drop(environment);
    drop(context);
    drop(retirement);
    drop(fixture.owner);
    drop(fixture.queued);
    drop(fixture.core);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    assert_eq!(lock(&core.state).bytes, 0);
    Ok(())
}
