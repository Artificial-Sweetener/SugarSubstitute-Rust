// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve unheld partial release and require real completion consumption before retirement.
use super::super::*;
use super::support::{Fake, Harness, TestResult};
use std::error::Error;
use sugarsubstitute_contracts::execution::{JobContext, Moment};

/// Model a native root-close failure after the original file exclusion has closed.
struct PartialRootClose {
    filesystem: Arc<Fake>,
    failure: Mutex<Option<ClaimError>>,
}
impl storage::ProfilePort for PartialRootClose {
    fn acquire(
        &self,
        state: &mut storage::NativeState,
        path: &std::path::Path,
        run: RunKey,
        context: &JobContext,
    ) -> Result<(), ClaimError> {
        self.filesystem.acquire(state, path, run, context)
    }
    fn validate_environment(
        &self,
        state: &mut storage::NativeState,
        observed: &[u16],
        context: &JobContext,
    ) -> Result<(), ClaimError> {
        self.filesystem.validate_environment(state, observed, context)
    }
    fn release(
        &self,
        state: &mut storage::NativeState,
        context: &JobContext,
    ) -> Result<(), ClaimError> {
        storage::check(context)?;
        if let Some(cause) = lock(&self.failure).take() {
            state.held = false;
            state.release_published = true;
            return Err(cause);
        }
        self.filesystem.release(state, context)
    }
}

#[test]
fn unheld_partial_release_can_finish_with_same_cutoff_and_retains_first_outcome() -> TestResult {
    let mut harness = Harness::new()?;
    assert_eq!(harness.acquire()?.kind, TerminalKind::Success);
    let original = ClaimError::new(
        "external_root_close",
        std::io::Error::other("Original partial native close"),
    );
    let original_source = std::ptr::from_ref(original.source().ok_or("Original source absent")?);
    harness.lease.port = Arc::new(PartialRootClose {
        filesystem: harness.fake.clone(),
        failure: Mutex::new(Some(original)),
    });
    let first = harness.release(100)?;
    assert_eq!(first.kind, TerminalKind::Failure);
    assert!(!harness.lease.snapshot().held);
    assert!(!harness.lease.snapshot().pending);
    assert!(!harness.lease.snapshot().retirement_ready());
    assert_eq!(
        std::ptr::from_ref(
            first
                .failure
                .as_ref()
                .ok_or("First cause absent")?
                .source()
                .ok_or("First source absent")?
        ),
        original_source
    );

    let second_spec = harness.spec(100)?;
    harness.lease.submit_release(
        &harness.client,
        &harness.scope,
        second_spec,
        &harness.registry,
    )?;
    let batch = harness.terminal_batch()?;
    assert!(harness.lease.snapshot().released);
    assert!(harness.lease.snapshot().pending);
    assert!(!harness.lease.snapshot().retirement_ready());
    assert_eq!(harness.lease.release_deadline, Some(Moment::from_nanos(100)));
    let second = harness.lease.consume(&batch)?.ok_or("Original second completion absent")?;
    assert_eq!(second.job, second_spec.id);
    assert_eq!(second.kind, TerminalKind::Success);
    assert!(second.failure.is_none());
    assert!(harness.lease.snapshot().retirement_ready());
    assert_eq!(
        std::ptr::from_ref(
            first
                .failure
                .as_ref()
                .ok_or("Retained first cause absent")?
                .source()
                .ok_or("Retained first source absent")?
        ),
        original_source
    );
    assert_eq!(harness.fake.releases.load(Ordering::Acquire), 1);
    harness.finish_workers();
    Ok(())
}

#[test]
fn actual_released_atomic_does_not_retire_an_unconsumed_owned_job() -> TestResult {
    let mut harness = Harness::new()?;
    assert_eq!(harness.acquire()?.kind, TerminalKind::Success);
    let spec = harness.spec(100)?;
    harness.lease.submit_release(&harness.client, &harness.scope, spec, &harness.registry)?;
    let batch = harness.terminal_batch()?;
    assert!(harness.lease.snapshot().released);
    assert!(!harness.lease.snapshot().retirement_ready());
    let rejected = harness.spec(100)?;
    assert!(matches!(
        harness.lease.submit_release(&harness.client, &harness.scope, rejected, &harness.registry),
        Err(ClaimAdmissionError::Busy)
    ));
    let completion = harness.lease.consume(&batch)?.ok_or("Original release completion absent")?;
    assert_eq!(completion.job, spec.id);
    assert_eq!(completion.kind, TerminalKind::Success);
    assert!(harness.lease.snapshot().retirement_ready());
    harness.finish_workers();
    Ok(())
}

#[test]
fn completed_profile_job_drains_despite_original_custody_transport_failure() -> TestResult {
    use crate::backend_custody_parent::observation_fixture;
    use sugarsubstitute_contracts::{backend_custody::*, execution::*};
    let mut h = Harness::new()?;
    let release = h.spec(100)?;
    h.lease.submit_release(&h.client, &h.scope, release, &h.registry)?;
    let batch = h.terminal_batch()?;
    assert!(h.lease.snapshot().released && h.lease.snapshot().pending);
    let native =
        h.client.create_native_scope(ScopeSpec { id: ScopeId::new(3)?, deadline: None }, None)?;
    let mut spec = h.spec(100)?;
    spec.class = JobClass::NativeObservation;
    spec.deadline = None;
    spec.reservation_bytes =
        sugarsubstitute_execution::windows_lifecycle_pipe::MIN_LIFECYCLE_PIPE_BYTES;
    let parent = observation_fixture::parent(
        &h.client,
        &native,
        spec,
        BackendCustodyKey::new(h.lease.run.owner, BackendCustodySession::new(1)?),
        Moment::from_nanos(30_000_000_000),
    )?;
    let cause = sugarsubstitute_execution::windows::NativeObservationError::new(
        "original_profile_transport",
        std::io::Error::other("Original transport failure"),
    );
    let pointer = std::ptr::from_ref(cause.source().ok_or("Original cause absent")?);
    let first = observation_fixture::fail(&parent, cause)?;
    lock(&h.lease.shared.native).as_mut().ok_or("Profile state absent")?.custody_release =
        Some(custody::Release {
            parent: parent.clone(),
            key: ProfileCustodyKey::new(h.lease.run, 2)?,
            until: None,
            nonce: None,
            counter: None,
            ack: None,
        });
    let observed = h.lease.consume(&batch);
    assert!(!h.lease.snapshot().pending, "Original completed job starved: {observed:?}");
    let completed = observed?.ok_or("Original profile completion lost")?;
    assert_eq!(completed.job, release.id);
    assert_eq!(completed.kind, TerminalKind::Success);
    let failure = completed.failure.ok_or("Original custody failure lost")?;
    let transport = failure
        .source()
        .and_then(|e| e.downcast_ref::<crate::BackendCustodyParentFailure>())
        .and_then(Error::source)
        .and_then(|e| {
            e.downcast_ref::<sugarsubstitute_execution::windows::NativeObservationError>()
        })
        .ok_or("Original transport type lost")?;
    assert_eq!(std::ptr::from_ref(transport.source().ok_or("Original source lost")?), pointer);
    assert_eq!(h.lease.release_deadline, Some(Moment::from_nanos(100)));
    assert!(h.lease.consume(&batch).is_err());
    drop(first);
    drop(parent);
    h.finish_workers();
    Ok(())
}

#[test]
fn actual_failed_profile_job_keeps_worker_primary_and_original_transport_secondary() -> TestResult {
    use crate::backend_custody_parent::observation_fixture;
    use sugarsubstitute_contracts::{backend_custody::*, execution::*};
    let mut h = Harness::new()?;
    assert!(lock(&h.lease.shared.native).as_ref().ok_or("Initial profile absent")?.retired);
    let acquired = h.acquire()?;
    assert_eq!(acquired.kind, TerminalKind::Success);
    assert!(acquired.failure.is_none());
    assert!(h.lease.snapshot().held);
    assert!(!lock(&h.lease.shared.native).as_ref().ok_or("Acquired profile absent")?.retired);
    h.fake.fail_release.store(true, Ordering::Release);
    let release = h.spec(100)?;
    h.lease.submit_release(&h.client, &h.scope, release, &h.registry)?;
    let batch = h.terminal_batch()?;
    assert_eq!(h.fake.releases.load(Ordering::Acquire), 1);
    let primary = lock(&h.lease.shared.native)
        .as_ref()
        .and_then(|state| state.failure.clone())
        .ok_or("Original worker cause absent")?;
    assert_eq!(primary.operation(), "fake_close_failure");
    let native =
        h.client.create_native_scope(ScopeSpec { id: ScopeId::new(3)?, deadline: None }, None)?;
    let mut spec = h.spec(100)?;
    spec.class = JobClass::NativeObservation;
    spec.deadline = None;
    spec.reservation_bytes =
        sugarsubstitute_execution::windows_lifecycle_pipe::MIN_LIFECYCLE_PIPE_BYTES;
    let parent = observation_fixture::parent(
        &h.client,
        &native,
        spec,
        BackendCustodyKey::new(h.lease.run.owner, BackendCustodySession::new(7)?),
        Moment::from_nanos(30_000_000_000),
    )?;
    let cause = sugarsubstitute_execution::windows::NativeObservationError::new(
        "original_parallel_transport",
        std::io::Error::from_raw_os_error(109),
    );
    let pointer = std::ptr::from_ref(cause.source().ok_or("Original transport source absent")?);
    let first = observation_fixture::fail(&parent, cause)?;
    lock(&h.lease.shared.native).as_mut().ok_or("Actual profile state absent")?.custody_release =
        Some(custody::Release {
            parent: parent.clone(),
            key: ProfileCustodyKey::new(h.lease.run, 2)?,
            until: None,
            nonce: None,
            counter: None,
            ack: None,
        });
    let completed = h.lease.consume(&batch)?.ok_or("Original terminal result absent")?;
    assert!(!h.lease.snapshot().pending);
    assert!(!h.lease.snapshot().retirement_ready());
    assert_eq!(completed.job, release.id);
    assert_eq!(completed.kind, TerminalKind::Failure);
    let failure = completed.failure.ok_or("Actual failed job became healthy")?;
    assert!(failure.same_primary(&primary));
    let secondary = failure.cleanup_cause().ok_or("Observed transport cause discarded")?;
    assert_eq!(secondary.operation(), "profile_release_observation");
    let original = secondary
        .source()
        .and_then(|cause| cause.downcast_ref::<crate::BackendCustodyParentFailure>())
        .and_then(Error::source)
        .and_then(|cause| {
            cause.downcast_ref::<sugarsubstitute_execution::windows::NativeObservationError>()
        })
        .ok_or("Original secondary type replaced")?;
    assert_eq!(
        std::ptr::from_ref(original.source().ok_or("Original secondary source lost")?),
        pointer
    );
    assert_eq!(h.lease.release_deadline, Some(Moment::from_nanos(100)));
    drop(first);
    drop(parent);
    h.finish_workers();
    Ok(())
}
