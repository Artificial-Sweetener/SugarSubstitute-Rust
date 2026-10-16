// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise actual admitted-claim retention without manufacturing a lease or authority.
use super::*;
use std::{
    cell::Cell,
    num::NonZeroUsize,
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};
use sugarsubstitute_contracts::execution::*;
use sugarsubstitute_execution::ExecutionOwner;
type TestResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;
struct ClockPort;
impl Clock for ClockPort {
    fn now(&self) -> Moment {
        Moment::from_nanos(1)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
#[derive(Default)]
struct Wake {
    sequence: Mutex<u64>,
    changed: Condvar,
}
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        let mut sequence = self.sequence.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        *sequence += 1;
        self.changed.notify_all();
        Ok(())
    }
}
/// Same real execution producer used by constructor storage tests. The worker
/// completes and runtime joins before either returned allocation guard is tested.
/// Separate admitted reservation and claimed partition exercise the unchanged4MiB role.
pub(in crate::production::entry_activation) fn admitted(
    reserved: u64,
    bytes: u64,
) -> Result<(MemoryClaim, Scope), Box<dyn std::error::Error + Send + Sync>> {
    let wake = Arc::new(Wake::default());
    let executor = ExecutionOwner::new(Arc::new(ClockPort), wake.clone())?;
    let client = executor.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut job = client.submit_blocking(
        &scope,
        JobSpec {
            id: JobId::new(1)?,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureOrdinary,
            deadline: Some(Moment::from_nanos(30_000_000_000)),
            reservation_bytes: reserved,
            replacement: None,
        },
        move |context| context.memory().claim(bytes),
    )?;
    let completion = loop {
        let observed = *wake.sequence.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(done) = job.try_take_completion() {
            break done;
        }
        client.drain_completions(NonZeroUsize::MIN)?;
        if let Some(done) = job.try_take_completion() {
            break done;
        }
        let sequence = wake.sequence.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let (sequence, timeout) = wake
            .changed
            .wait_timeout_while(sequence, Duration::from_secs(10), |value| *value == observed)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        drop(sequence);
        if timeout.timed_out() {
            return Err("Actual funding completion boundary timed out".into());
        }
    };
    let claim = match completion.outcome {
        JobOutcome::Success(claim) => claim,
        JobOutcome::Failure(cause) => return Err(cause.into()),
        _ => return Err("Actual funding job did not succeed".into()),
    };
    drop(job);
    drop(executor);
    Ok((claim, scope))
}
#[test]
fn exact_quota_and_either_alias_keep_original_after_ack_and_worker_join() -> TestResult {
    for drop_data_first in [false, true] {
        let (original, scope) = admitted(64 * 1024, 64 * 1024)?;
        let mut original = Some(original);
        let (data, credit) =
            complete(&mut original, ProjectionDataBytes { retained: 4096, future: 2048 })?;
        assert!(original.is_none());
        assert_eq!(credit.bytes(), 64 * 1024 - 4096 - 2048 - backing_bytes());
        assert_eq!(scope.snapshot().reserved_bytes, 64 * 1024);
        if drop_data_first {
            drop(data);
            assert_eq!(scope.snapshot().reserved_bytes, 64 * 1024);
            drop(credit);
        } else {
            drop(credit);
            assert_eq!(scope.snapshot().reserved_bytes, 64 * 1024);
            drop(data);
        }
        assert_eq!(scope.snapshot().reserved_bytes, 0);
    }
    Ok(())
}
#[test]
fn unavailable_or_overflowed_data_leaves_original_claim_and_creates_no_credit() -> TestResult {
    let (original, scope) = admitted(8192, 8192)?;
    let mut original = Some(original);
    assert!(matches!(
        complete(&mut original, ProjectionDataBytes { retained: u64::MAX, future: 16 }),
        Err(LifecycleRejection::Overloaded)
    ));
    assert_eq!(original.as_ref().ok_or("Original charge lost")?.bytes(), 8192);
    assert_eq!(scope.snapshot().reserved_bytes, 8192);
    assert!(matches!(
        complete(&mut original, ProjectionDataBytes { retained: 8192, future: 1 }),
        Err(LifecycleRejection::Overloaded)
    ));
    drop(original);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
struct ObservedData<'a> {
    scope: &'a Scope,
    dropped: &'a Cell<bool>,
    bytes: Vec<u8>,
    _funding: ProjectionDataFunding,
}
impl Drop for ObservedData<'_> {
    fn drop(&mut self) {
        assert_eq!(self.scope.snapshot().reserved_bytes, 64 * 1024);
        assert_eq!(self.bytes.len(), 4096);
        self.dropped.set(true);
    }
}
#[test]
fn actual_data_deallocates_before_the_final_backing_returns_credit() -> TestResult {
    let (original, scope) = admitted(64 * 1024, 64 * 1024)?;
    let mut original = Some(original);
    let (data, credit) =
        complete(&mut original, ProjectionDataBytes { retained: 4096, future: 2048 })?;
    let dropped = Cell::new(false);
    let value =
        ObservedData { scope: &scope, dropped: &dropped, bytes: vec![0; 4096], _funding: data };
    drop(credit);
    drop(value);
    assert!(dropped.get());
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn projection_partition_follows_actual_containing_owner_growth() -> TestResult {
    use crate::production::entry_activation::{
        ParentLifecycle, Record, RetainedBackend,
        delivery::TransitionDelivery,
        projection::{PROJECTION_BYTES, ProjectionInput, projection_data_bytes},
        recovery_supervisor::RecoveryPreparation,
    };
    let remaining = projection_data_bytes().ok_or("Actual projection roles do not fit")?;
    let sum = remaining
        + ParentLifecycle::storage_bytes()
        + size_of::<Record>() as u64
        + RetainedBackend::required_storage_bytes()
        + TransitionDelivery::storage_bytes()
        + size_of::<ProjectionInput>().max(size_of::<RecoveryPreparation>()) as u64
        + sugarsubstitute_execution::REGISTERED_STARTUP_BYTES;
    assert_eq!(sum, PROJECTION_BYTES);
    assert_eq!(PROJECTION_BYTES, 4 * 1024 * 1024);
    assert_eq!(ParentLifecycle::storage_bytes(), 2 * 1024 * 1024);
    Ok(())
}

#[test]
fn backend_fixture_credit_retains_the_real_empty_data_claim_until_final_drop() -> TestResult {
    let (original, scope) = admitted(8192, 8192)?;
    let credit = test_policy_credit(original)?;
    assert_eq!(credit.bytes(), 8192 - backing_bytes());
    assert_eq!(scope.snapshot().reserved_bytes, 8192);
    drop(credit);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

/// Characterize the same original projection reservation after the worker input
/// retired. Private registered/capture capabilities are not fabricated: their
/// exact co-resident quota remains claimed, alongside the real inert Parent.
#[test]
fn fresh_recovery_preparation_fits_with_all_original_projection_roles_live() -> TestResult {
    use crate::production::entry_activation::{
        ParentLifecycle, Record, RetainedBackend, RetainedRecord,
        delivery::TransitionDelivery,
        projection::{PROJECTION_BYTES, ProjectionInput, projection_data_bytes},
        recovery_supervisor::RecoveryPreparation,
    };
    use crate::production::parent_lifecycle::causes::BackendCauses;
    use sugarsubstitute_contracts::{configuration::ProfileId, lifecycle::*, startup::*};
    let wake = Arc::new(Wake::default());
    let executor = ExecutionOwner::new(Arc::new(ClockPort), wake.clone())?;
    let client = executor.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut job = client.submit_blocking(
        &scope,
        JobSpec {
            id: JobId::new(1)?,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureOrdinary,
            deadline: Some(Moment::from_nanos(30_000_000_000)),
            reservation_bytes: PROJECTION_BYTES,
            replacement: None,
        },
        |context| Ok::<_, std::io::Error>(context.memory().clone()),
    )?;
    let completion = loop {
        let observed = *wake.sequence.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(done) = job.try_take_completion() {
            break done;
        }
        client.drain_completions(NonZeroUsize::MIN)?;
        if let Some(done) = job.try_take_completion() {
            break done;
        }
        let sequence = wake.sequence.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let (sequence, timeout) = wake
            .changed
            .wait_timeout_while(sequence, Duration::from_secs(10), |value| *value == observed)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        drop(sequence);
        if timeout.timed_out() {
            return Err("Actual recovery funding admission completion timed out".into());
        }
    };
    let memory = match completion.outcome {
        JobOutcome::Success(memory) => memory,
        JobOutcome::Failure(cause) => return Err(cause.into()),
        _ => return Err("Actual recovery funding reservation did not succeed".into()),
    };
    drop(completion.memory);
    drop(completion.native_handles);
    drop(job);
    drop(executor);
    let original_data = memory.claim(projection_data_bytes().ok_or("Actual partition absent")?)?;
    let mut original_data = Some(original_data);
    let (data, credit) =
        complete(&mut original_data, ProjectionDataBytes { retained: 4096, future: 2048 })?;
    assert!(original_data.is_none());
    assert!(credit.bytes() > 0);
    let backend = memory.claim(RetainedBackend::required_storage_bytes())?;
    let delivery = memory.claim(TransitionDelivery::storage_bytes())?;
    let registered = memory.claim(sugarsubstitute_execution::REGISTERED_STARTUP_BYTES)?;
    let record_storage = memory.claim(size_of::<Record>() as u64)?;
    let mut causes = Some(BackendCauses::new(memory.claim(ParentLifecycle::storage_bytes())?)?);
    let run = RunKey {
        owner: CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16])?,
        },
        instance: InstanceId::new(1)?,
        generation: RunGeneration::new(1)?,
    };
    let launch = LaunchId::new(1)?;
    let connection = ConnectionId::new(1)?;
    let parent = Box::new(ParentLifecycle::new(
        run,
        launch,
        ProfileId::new("Funding_1".into())?,
        connection,
        sugarsubstitute_execution::windows_lifecycle_pipe::LifecyclePipeDeadline::new(
            100,
            1_000_000_000,
        )?,
        &mut causes,
    )?);
    assert!(causes.is_none());
    assert_eq!(parent.cause_reserved_for_owner_test()?, PROJECTION_BYTES);
    let mut record = Box::new(Record::new(
        MainIdentity { run, launch, navigation: Generation::new(1)? },
        connection,
        Default::default(),
        true,
        false,
    ));
    record.parent = Some(parent);
    let record = RetainedRecord { record, _storage: record_storage };
    // This is the exact current constructor's claim for its inner Box payload,
    // including its inline completion/frame/operation fields. The outer Root
    // field remains in its existing containing allocation. Arc backing is live.
    let recovery_storage = memory.claim(size_of::<RecoveryPreparation>() as u64)?;
    let recovery_payload = vec![0_u8; size_of::<RecoveryPreparation>()].into_boxed_slice();
    assert_eq!(recovery_payload.len() as u64, recovery_storage.bytes());
    assert_eq!(scope.snapshot().reserved_bytes, PROJECTION_BYTES);
    assert!(size_of::<RecoveryPreparation>() > size_of::<ProjectionInput>());
    drop(recovery_payload);
    drop(recovery_storage);
    drop(record);
    drop(registered);
    drop(delivery);
    drop(backend);
    drop(data);
    assert_eq!(scope.snapshot().reserved_bytes, PROJECTION_BYTES);
    drop(credit);
    drop(memory);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
