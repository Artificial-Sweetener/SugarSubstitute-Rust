// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise registration-lifetime replay and real admitted storage without native effects.
use super::*;
use std::{
    num::NonZeroUsize,
    sync::{Condvar, Mutex},
};
use sugarsubstitute_contracts::{execution::*, lifecycle::*};
use sugarsubstitute_execution::ExecutionOwner;
type TestResult<T = ()> = Result<T, Box<dyn Error>>;

struct ClockBoundary;
impl Clock for ClockBoundary {
    fn now(&self) -> Moment {
        Moment::from_nanos(0)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
#[derive(Default)]
struct WakeBoundary {
    count: Mutex<u64>,
    changed: Condvar,
}
impl CompletionWake for WakeBoundary {
    fn wake(&self) -> Result<(), ExecutionError> {
        *lock(&self.count) += 1;
        self.changed.notify_all();
        Ok(())
    }
}
struct Harness {
    _owner: ExecutionOwner,
    scope: Scope,
    attempts: Box<Attempts>,
}
impl Harness {
    fn new() -> TestResult<Self> {
        let wake = Arc::new(WakeBoundary::default());
        let owner = ExecutionOwner::new(Arc::new(ClockBoundary), wake.clone())?;
        let client = owner.client();
        let scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        let spec = JobSpec {
            id: JobId::new(1)?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(Moment::from_nanos(1)),
            reservation_bytes: SOURCE_BYTES,
            replacement: None,
        };
        let key = coordinator()?;
        let mut job = client.submit_blocking(&scope, spec, move |context| {
            let memory =
                context.memory().claim(SOURCE_BYTES).map_err(|_| SourceFailure::Storage)?;
            Attempts::new(key, memory)
        })?;
        let completion = loop {
            let previous = *lock(&wake.count);
            if let Some(completion) = job.try_take_completion() {
                break completion;
            }
            client.drain_completions(NonZeroUsize::MIN)?;
            if let Some(completion) = job.try_take_completion() {
                break completion;
            }
            drop(
                wake.changed
                    .wait_while(lock(&wake.count), |count| *count == previous)
                    .map_err(|_| "Poisoned controlled wake")?,
            );
        };
        let JobOutcome::Success(attempts) = completion.outcome else {
            return Err("Actual storage allocation failed".into());
        };
        drop(completion.memory);
        drop(job);
        assert_eq!(scope.snapshot().reserved_bytes, SOURCE_BYTES);
        Ok(Self { _owner: owner, scope, attempts })
    }
}
fn coordinator() -> TestResult<CoordinatorKey> {
    Ok(CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    })
}
fn run(instance: u64, generation: u64) -> TestResult<RunKey> {
    Ok(RunKey {
        owner: coordinator()?,
        instance: InstanceId::new(instance)?,
        generation: RunGeneration::new(generation)?,
    })
}
fn budget(start: u64) -> TestResult<CaptureBudget> {
    Ok(CaptureBudget::new(Moment::from_nanos(start))?)
}
fn binding(job: u64) -> TestResult<(ExecutionId, JobId)> {
    Ok((ExecutionId::new(3)?, JobId::new(job)?))
}

#[test]
fn actual_source_and_all_sixty_four_view_layouts_fit_the_unchanged_claim() -> TestResult {
    let harness = Harness::new()?;
    let actual =
        size_of::<Attempts>() + SLOTS * (size_of::<RegisteredSource>() + 2 * size_of::<usize>());
    assert!(actual <= SOURCE_BYTES as usize);
    assert_eq!(harness.attempts._memory.bytes(), 16_384);
    let Harness { _owner, scope, attempts } = harness;
    drop(attempts);
    drop(_owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn full_run_replay_cannot_renew_budget_after_job_or_view_retirement() -> TestResult {
    let mut harness = Harness::new()?;
    let first = harness.attempts.reserve(CaptureTarget::Desktop(run(1, 1)?), budget(0)?)?;
    let (execution, job) = binding(1)?;
    harness.attempts.commit(first, execution, job)?;
    harness.attempts.release(first);
    assert!(matches!(
        harness.attempts.reserve(CaptureTarget::Desktop(run(1, 1)?), budget(10)?),
        Err(SourceFailure::Replay)
    ));
    // Physical retirement alone cannot replace an outstanding source view.
    let second = harness.attempts.reserve(CaptureTarget::Desktop(run(2, 1)?), budget(0)?)?;
    harness.attempts.commit(second, execution, JobId::new(2)?)?;
    harness.attempts.retire(second, execution, JobId::new(2)?)?;
    assert!(matches!(
        harness.attempts.reserve(CaptureTarget::Desktop(run(2, 2)?), budget(10)?),
        Err(SourceFailure::Replay)
    ));
    harness.attempts.release(second);
    let next = harness.attempts.reserve(CaptureTarget::Desktop(run(2, 2)?), budget(10)?)?;
    assert_eq!(next.budget, budget(10)?);
    harness.attempts.release(next);
    assert!(matches!(
        harness.attempts.reserve(CaptureTarget::Desktop(run(2, 1)?), budget(10)?),
        Err(SourceFailure::Replay)
    ));
    Ok(())
}
#[test]
fn rejected_pending_admission_restores_retired_generation_and_exact_retry() -> TestResult {
    let mut harness = Harness::new()?;
    let (execution, job) = binding(1)?;
    let first = harness.attempts.reserve(CaptureTarget::Desktop(run(1, 1)?), budget(0)?)?;
    harness.attempts.commit(first, execution, job)?;
    harness.attempts.retire(first, execution, job)?;
    harness.attempts.release(first);
    let pending = harness.attempts.reserve(CaptureTarget::Desktop(run(1, 2)?), budget(20)?)?;
    assert!(matches!(
        harness.attempts.reserve(CaptureTarget::Desktop(run(1, 2)?), budget(30)?),
        Err(SourceFailure::Replay)
    ));
    harness.attempts.release(pending);
    assert!(matches!(
        harness.attempts.reserve(CaptureTarget::Desktop(run(1, 1)?), budget(30)?),
        Err(SourceFailure::Replay)
    ));
    let retry = harness.attempts.reserve(CaptureTarget::Desktop(run(1, 2)?), budget(20)?)?;
    assert!(retry.sequence > pending.sequence);
    assert_eq!(retry.budget, pending.budget);
    assert!(matches!(
        harness.attempts.commit(pending, execution, job),
        Err(SourceFailure::ForeignAssociation)
    ));
    harness.attempts.release(retry);
    Ok(())
}
#[test]
fn exact_late_producer_commit_preserves_already_retired_binding() -> TestResult {
    let mut harness = Harness::new()?;
    let (execution, job) = binding(1)?;
    let view = harness.attempts.reserve(CaptureTarget::Desktop(run(1, 1)?), budget(0)?)?;
    harness.attempts.commit(view, execution, job)?;
    harness.attempts.retire(view, execution, job)?;
    harness.attempts.commit(view, execution, job)?;
    let actual = harness.attempts.exact(view)?;
    assert_eq!(actual.phase, Phase::NativeRetired(execution, job));
    assert_eq!(actual.budget, budget(0)?);
    assert!(matches!(
        harness.attempts.commit(view, execution, JobId::new(2)?),
        Err(SourceFailure::ForeignAssociation)
    ));
    assert!(matches!(
        harness.attempts.retire(view, ExecutionId::new(4)?, job),
        Err(SourceFailure::ForeignAssociation)
    ));
    assert_eq!(harness.attempts.instance_highwater, 1);
    harness.attempts.release(view);
    Ok(())
}
#[test]
fn bounded_eviction_requires_retirement_and_restores_rejected_victim() -> TestResult {
    let mut harness = Harness::new()?;
    let (execution, _) = binding(1)?;
    let mut views = Vec::new();
    for instance in 1..=64 {
        let view =
            harness.attempts.reserve(CaptureTarget::Desktop(run(instance, 1)?), budget(0)?)?;
        harness.attempts.commit(view, execution, JobId::new(instance)?)?;
        harness.attempts.retire(view, execution, JobId::new(instance)?)?;
        views.push(view);
    }
    assert!(matches!(
        harness.attempts.reserve(CaptureTarget::Desktop(run(65, 1)?), budget(0)?),
        Err(SourceFailure::Overloaded)
    ));
    harness.attempts.release(views[0]);
    let pending = harness.attempts.reserve(CaptureTarget::Desktop(run(65, 1)?), budget(0)?)?;
    harness.attempts.release(pending);
    assert_eq!(harness.attempts.slots[0].map(|slot| slot.instance), Some(1));
    let retry = harness.attempts.reserve(CaptureTarget::Desktop(run(65, 1)?), budget(0)?)?;
    harness.attempts.commit(retry, execution, JobId::new(65)?)?;
    harness.attempts.retire(retry, execution, JobId::new(65)?)?;
    harness.attempts.release(retry);
    assert!(matches!(
        harness.attempts.reserve(CaptureTarget::Desktop(run(1, 2)?), budget(100)?),
        Err(SourceFailure::Replay)
    ));
    for view in &views[1..] {
        harness.attempts.release(*view);
    }
    for instance in 66..=600 {
        let view =
            harness.attempts.reserve(CaptureTarget::Desktop(run(instance, 1)?), budget(0)?)?;
        harness.attempts.commit(view, execution, JobId::new(instance)?)?;
        harness.attempts.retire(view, execution, JobId::new(instance)?)?;
        harness.attempts.release(view);
    }
    assert_eq!(harness.attempts.slots.iter().flatten().count(), 64);
    assert_eq!(harness.attempts.instance_highwater, 600);
    Ok(())
}
#[test]
fn complete_owner_closing_and_exhaustion_fences_preserve_original_authority() -> TestResult {
    let mut harness = Harness::new()?;
    let mut foreign = run(1, 1)?;
    foreign.owner.epoch = CoordinatorEpoch::new([3; 16])?;
    assert!(matches!(
        harness.attempts.reserve(CaptureTarget::Desktop(foreign), budget(0)?),
        Err(SourceFailure::ForeignOwner)
    ));
    let view = harness.attempts.reserve(CaptureTarget::Desktop(run(1, 1)?), budget(0)?)?;
    let (execution, job) = binding(1)?;
    harness.attempts.commit(view, execution, job)?;
    harness.attempts.close();
    assert!(matches!(
        harness.attempts.reserve(CaptureTarget::Desktop(run(2, 1)?), budget(0)?),
        Err(SourceFailure::Closing)
    ));
    assert!(harness.attempts.has_unretired());
    harness.attempts.retire(view, execution, job)?;
    assert!(!harness.attempts.has_unretired());
    harness.attempts.release(view);
    let mut second = Harness::new()?;
    second.attempts.sequence = u64::MAX;
    assert!(matches!(
        second.attempts.reserve(CaptureTarget::Desktop(run(1, 1)?), budget(0)?),
        Err(SourceFailure::Exhausted)
    ));
    assert!(second.attempts.slots.iter().all(Option::is_none));
    assert_eq!(second.attempts.sequence, u64::MAX);
    Ok(())
}

fn backend_target(session: u64) -> TestResult<CaptureTarget> {
    use sugarsubstitute_contracts::backend_custody::{BackendCustodyKey, BackendCustodySession};
    Ok(CaptureTarget::BackendCustody(BackendCustodyKey::new(
        coordinator()?,
        BackendCustodySession::new(session)?,
    )))
}
#[test]
fn desktop_and_backend_targets_share_the_original_sixty_four_slot_capacity() -> TestResult {
    let mut harness = Harness::new()?;
    let helper = harness.attempts.reserve(backend_target(1)?, budget(0)?)?;
    for instance in 1..64 {
        harness.attempts.reserve(CaptureTarget::Desktop(run(instance, 1)?), budget(0)?)?;
    }
    assert_eq!(harness.attempts.slots.iter().flatten().count(), 64);
    assert!(matches!(
        harness.attempts.reserve(backend_target(2)?, budget(0)?),
        Err(SourceFailure::Overloaded)
    ));
    assert!(matches!(
        harness.attempts.reserve(CaptureTarget::Desktop(run(64, 1)?), budget(0)?),
        Err(SourceFailure::Overloaded)
    ));
    let (execution, job) = binding(3)?;
    harness.attempts.commit(helper, execution, job)?;
    harness.attempts.retire(helper, execution, job)?;
    harness.attempts.release(helper);
    assert!(matches!(
        harness.attempts.reserve(backend_target(1)?, budget(1)?),
        Err(SourceFailure::Replay)
    ));
    harness.attempts.reserve(backend_target(2)?, budget(0)?)?;
    assert_eq!(harness.attempts._memory.bytes(), 16_384);
    Ok(())
}
#[test]
fn helper_target_highwater_and_retirement_preserve_the_original_identity() -> TestResult {
    let mut harness = Harness::new()?;
    let helper = harness.attempts.reserve(backend_target(100)?, budget(0)?)?;
    let desktop = harness.attempts.reserve(CaptureTarget::Desktop(run(1, 1)?), budget(0)?)?;
    let (execution, job) = binding(3)?;
    harness.attempts.commit(helper, execution, job)?;
    assert_eq!(harness.attempts.backend_highwater, 100);
    assert_eq!(harness.attempts.instance_highwater, 0);
    assert!(harness.attempts.has_unretired());
    harness.attempts.retire(helper, execution, job)?;
    harness.attempts.release(helper);
    assert!(matches!(
        harness.attempts.reserve(backend_target(99)?, budget(0)?),
        Err(SourceFailure::Replay)
    ));
    harness.attempts.commit(desktop, execution, JobId::new(4)?)?;
    assert_eq!(harness.attempts.instance_highwater, 1);
    harness.attempts.retire(desktop, execution, JobId::new(4)?)?;
    harness.attempts.release(desktop);
    assert!(!harness.attempts.has_unretired());
    harness.attempts.reserve(CaptureTarget::Desktop(run(1, 2)?), budget(0)?)?;
    Ok(())
}
