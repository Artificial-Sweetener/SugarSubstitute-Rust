// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Exercise actual paired admission, pre-held worker custody and immutable cleanup activation.

use super::*;
use crate::runtime_test_support::{Gate, Rig, TestResult, spec};
use std::{
    io,
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
use sugarsubstitute_contracts::execution::{
    ExecutionPort, JobClass, JobOutcome, SymbolNativePartition,
};

struct ReleaseGate(Arc<Gate>);
impl Drop for ReleaseGate {
    fn drop(&mut self) {
        self.0.open();
    }
}

fn setup(rig: &Rig) -> TestResult<(Scope, JobSpec, JobSpec)> {
    let cleanup =
        rig.client.create_scope(ScopeSpec { id: ScopeId::new(2)?, deadline: None }, None)?;
    let cleanup_spec =
        spec(1, JobClass::InfrastructureLifecycle, crate::symbol_admission::CLEANUP_BYTES)?;
    let mut main_spec =
        spec(2, JobClass::InfrastructureOrdinary, crate::symbol_admission::MAIN_BYTES)?;
    main_spec.deadline = Some(Moment::from_nanos(10_000_000_000));
    Ok((cleanup, cleanup_spec, main_spec))
}

fn accepted<T, M>(result: Result<T, crate::SymbolSubmissionRejection<M>>) -> TestResult<T> {
    result.map_err(|rejection| Box::new(rejection.error) as Box<dyn std::error::Error>)
}
fn begun<T: Send + 'static, E: Send + 'static>(
    result: Result<OwnedJob<T, E>, SymbolCleanupBeginRejection<T, E>>,
) -> TestResult<OwnedJob<T, E>> {
    result.map_err(|rejection| Box::new(rejection.error) as Box<dyn std::error::Error>)
}

#[test]
fn held_cleanup_has_real_queue_byte_and_terminal_credit_but_no_eligible_factory() -> TestResult {
    let rig = Rig::new()?;
    let (cleanup, cleanup_spec, main_spec) = setup(&rig)?;
    let effects = Arc::new(AtomicUsize::new(0));
    let called = effects.clone();
    let permit = accepted(rig.client.reserve_symbol_cleanup(
        &cleanup,
        cleanup_spec,
        &rig.scope,
        main_spec,
        move |_| {
            called.fetch_add(1, Ordering::AcqRel);
            Ok::<_, io::Error>(())
        },
    ))?;
    assert_eq!(permit.status().phase, JobPhase::Queued);
    assert_eq!(cleanup.snapshot().queued, 1);
    assert_eq!(cleanup.snapshot().reserved_bytes, 32_768);
    assert!(
        rig.client
            .core_for_test()
            .take(|class| class == JobClass::InfrastructureLifecycle)
            .is_none()
    );
    assert!(matches!(
        rig.client.freeze_symbol_cleanup_deadline(&permit, Duration::from_secs(15)),
        Err(AdmissionError::InvalidSymbolAdmission)
    ));
    let mut actual = permit.cancel_and_observe();
    let completion = rig.completion(&mut actual)?;
    assert!(matches!(completion.outcome, JobOutcome::Cancelled));
    assert_eq!(effects.load(Ordering::Acquire), 0);
    drop(completion);
    drop(actual);
    drop(rig.owner);
    assert_eq!(cleanup.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn paired_cleanup_reuses_actual_job_and_first_deadline_after_original_main_expiry() -> TestResult {
    let rig = Rig::new()?;
    let (cleanup, cleanup_spec, main_spec) = setup(&rig)?;
    let cleanup_context = Arc::new(Mutex::new(None));
    let observed = cleanup_context.clone();
    let permit = accepted(rig.client.reserve_symbol_cleanup(
        &cleanup,
        cleanup_spec,
        &rig.scope,
        main_spec,
        move |context| {
            *lock(&observed) = Some(context);
            Ok::<_, io::Error>(())
        },
    ))?;
    let mut main =
        accepted(
            rig.client
                .submit_symbol_helper(&rig.scope, main_spec, &permit, |_| Ok::<_, io::Error>(())),
        )?;
    let completion = rig.completion(&mut main)?;
    assert!(completion.native_handles.is_some());
    rig.clock.advance(10_000_000_000);
    let original = rig.client.freeze_symbol_cleanup_deadline(&permit, Duration::from_secs(15))?;
    assert_eq!(original.started(), Moment::from_nanos(10_000_000_000));
    assert_eq!(original.deadline(), Moment::from_nanos(25_000_000_000));
    rig.clock.advance(1_000_000_000);
    assert_eq!(
        rig.client.freeze_symbol_cleanup_deadline(&permit, Duration::from_secs(15))?,
        original
    );
    assert!(matches!(
        rig.client.freeze_symbol_cleanup_deadline(&permit, Duration::from_secs(14)),
        Err(AdmissionError::InvalidSymbolAdmission)
    ));
    let actual_id = permit.original_job_id();
    let mut activated = begun(permit.begin(original))?;
    assert_eq!(activated.spec(), cleanup_spec);
    assert_eq!(activated.spec().id, actual_id);
    let cleanup_completion = rig.completion(&mut activated)?;
    assert!(matches!(cleanup_completion.outcome, JobOutcome::Success(())));
    let context = lock(&cleanup_context).take().ok_or("Actual cleanup context absent")?;
    assert_eq!(context.job_id(), actual_id);
    assert_eq!(context.spec(), cleanup_spec);
    assert_eq!(context.deadline(), Some(original.deadline()));
    assert_eq!(context.memory().bytes(), 32_768);
    assert!(context.native_handles().is_none());
    drop(context);
    drop(cleanup_completion);
    drop(activated);
    drop(completion);
    drop(main);
    drop(rig.owner);
    assert_eq!(cleanup.snapshot().reserved_bytes, 0);
    assert_eq!(rig.scope.snapshot().native_reservations, 0);
    Ok(())
}

#[test]
fn reentrant_cleanup_clock_cancellation_vetoes_activation_and_returns_same_permit() -> TestResult {
    let rig = Rig::new()?;
    let (cleanup, cleanup_spec, main_spec) = setup(&rig)?;
    let permit = accepted(rig.client.reserve_symbol_cleanup(
        &cleanup,
        cleanup_spec,
        &rig.scope,
        main_spec,
        |_| Ok::<_, io::Error>(()),
    ))?;
    let mut main =
        accepted(
            rig.client
                .submit_symbol_helper(&rig.scope, main_spec, &permit, |_| Ok::<_, io::Error>(())),
        )?;
    let completion = rig.completion(&mut main)?;
    let original = rig.client.freeze_symbol_cleanup_deadline(&permit, Duration::from_secs(15))?;
    let client = rig.client.clone();
    let cleanup_id = cleanup.id();
    rig.clock.on_next_current_thread_observation(Box::new(move || {
        client.core_for_test().cancel_scope(cleanup_id)
    }));
    let rejection = match permit.begin(original) {
        Err(rejection) => rejection,
        Ok(_) => return Err("Cancelled cleanup activated".into()),
    };
    assert_eq!(rejection.permit.original_job_id(), cleanup_spec.id);
    assert_eq!(rejection.permit.original_scope_id(), cleanup.id());
    let mut same = rejection.permit.cancel_and_observe();
    assert!(matches!(rig.completion(&mut same)?.outcome, JobOutcome::Cancelled));
    drop(completion);
    Ok(())
}

#[test]
fn current_core_ledger_is_required_before_transport_partition_handoff() -> TestResult {
    let rig = Rig::new()?;
    let (cleanup, cleanup_spec, main_spec) = setup(&rig)?;
    let permit = accepted(rig.client.reserve_symbol_cleanup(
        &cleanup,
        cleanup_spec,
        &rig.scope,
        main_spec,
        |_| Ok::<_, io::Error>(()),
    ))?;
    let entered = Arc::new(Gate::default());
    let release = Arc::new(Gate::default());
    let _release_on_failure = ReleaseGate(release.clone());
    let stored = Arc::new(Mutex::new(None));
    let worker_stored = stored.clone();
    let worker_entered = entered.clone();
    let worker_release = release.clone();
    let mut main = accepted(rig.client.submit_symbol_helper(
        &rig.scope,
        main_spec,
        &permit,
        move |context| {
            *lock(&worker_stored) = Some(context);
            worker_entered.open();
            worker_release.wait().map_err(|cause| io::Error::other(cause.to_string()))?;
            Ok::<_, io::Error>(())
        },
    ))?;
    entered.wait()?;
    let context = lock(&stored).take().ok_or("Actual main context absent")?;
    let lease = context.native_handles().ok_or("Actual ledger absent")?;
    let transport = lease.take_partition(SymbolNativePartition::Transport)?;
    let actual = rig
        .client
        .symbol_transport_reservation(&context, transport)
        .map_err(|rejection| rejection.error)?;
    assert!(actual.handles.matches_lease(lease));
    assert_eq!(actual.handles.capacity(), 20);
    let output = actual.handles.reserve_output()?;
    assert_eq!(rig.scope.snapshot().native_reservations, 1);
    assert_eq!(rig.scope.snapshot().native_outputs_in_custody, 1);
    release.open();
    let completion = rig.completion(&mut main)?;
    drop(completion);
    drop(main);
    drop(actual);
    drop(context);
    let mut cleanup_job = permit.cancel_and_observe();
    drop(rig.completion(&mut cleanup_job)?);
    drop(cleanup_job);
    drop(rig.owner);
    assert_eq!(rig.scope.snapshot().native_reservations, 1);
    assert_eq!(rig.scope.snapshot().native_outputs_in_custody, 1);
    assert_eq!(rig.scope.snapshot().reserved_bytes, crate::symbol_admission::MAIN_BYTES);
    drop(output);
    assert_eq!(rig.scope.snapshot().native_reservations, 0);
    assert_eq!(rig.scope.snapshot().reserved_bytes, 0);
    Ok(())
}

struct HeldDestruction {
    entered: Arc<Gate>,
    release: Arc<Gate>,
    dropped: Arc<AtomicUsize>,
}
impl Drop for HeldDestruction {
    fn drop(&mut self) {
        self.entered.open();
        if self.release.wait().is_ok() {
            self.dropped.fetch_add(1, Ordering::AcqRel);
        }
    }
}
#[test]
fn cancellation_observes_actual_held_factory_destruction_before_terminal_or_byte_release()
-> TestResult {
    let rig = Rig::new()?;
    let (cleanup, cleanup_spec, main_spec) = setup(&rig)?;
    let entered = Arc::new(Gate::default());
    let release = Arc::new(Gate::default());
    let _release_on_failure = ReleaseGate(release.clone());
    let dropped = Arc::new(AtomicUsize::new(0));
    let captured = HeldDestruction {
        entered: entered.clone(),
        release: release.clone(),
        dropped: dropped.clone(),
    };
    let effects = Arc::new(AtomicUsize::new(0));
    let called = effects.clone();
    let permit = accepted(rig.client.reserve_symbol_cleanup(
        &cleanup,
        cleanup_spec,
        &rig.scope,
        main_spec,
        move |_| {
            called.fetch_add(1, Ordering::AcqRel);
            drop(captured);
            Ok::<_, io::Error>(())
        },
    ))?;
    let mut job = permit.cancel_and_observe();
    entered.wait()?;
    assert_eq!(job.status().phase, JobPhase::Running);
    assert!(job.try_take_completion().is_none());
    assert_eq!(cleanup.snapshot().reserved_bytes, 32_768);
    assert_eq!(effects.load(Ordering::Acquire), 0);
    assert_eq!(dropped.load(Ordering::Acquire), 0);
    release.open();
    let completion = rig.completion(&mut job)?;
    assert!(matches!(completion.outcome, JobOutcome::Cancelled));
    assert_eq!(dropped.load(Ordering::Acquire), 1);
    drop(completion);
    drop(job);
    drop(rig.owner);
    assert_eq!(cleanup.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn failed_main_returns_same_unexecuted_factory_and_leaves_observable_unused_cleanup() -> TestResult
{
    let rig = Rig::new()?;
    let (cleanup, cleanup_spec, main_spec) = setup(&rig)?;
    let permit = accepted(rig.client.reserve_symbol_cleanup(
        &cleanup,
        cleanup_spec,
        &rig.scope,
        main_spec,
        |_| Ok::<_, io::Error>(()),
    ))?;
    let effects = Arc::new(AtomicUsize::new(0));
    let called = effects.clone();
    let mut invalid = main_spec;
    invalid.reservation_bytes -= 1;
    let rejected = match rig.client.submit_symbol_helper(&rig.scope, invalid, &permit, move |_| {
        called.fetch_add(1, Ordering::AcqRel);
        Ok::<_, io::Error>(())
    }) {
        Err(rejected) => rejected,
        Ok(_) => return Err("Invalid fixed charge admitted".into()),
    };
    assert!(matches!(rejected.error, AdmissionError::InvalidSymbolAdmission));
    assert_eq!(effects.load(Ordering::Acquire), 0);
    assert_eq!(permit.original_job_id(), cleanup_spec.id);
    assert_eq!(permit.status().phase, JobPhase::Queued);
    assert!(rig.client.freeze_symbol_cleanup_deadline(&permit, Duration::from_secs(15)).is_err());
    drop(rejected.factory);
    let mut actual = permit.cancel_and_observe();
    let completion = rig.completion(&mut actual)?;
    assert!(matches!(completion.outcome, JobOutcome::Cancelled));
    assert_eq!(effects.load(Ordering::Acquire), 0);
    Ok(())
}

#[path = "symbol_cleanup_retirement_tests.rs"]
mod retirement;
