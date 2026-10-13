// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retire actual failed initialization and retained pool without a fabricated registry.
use super::*;
use crate::native_claims::RecordState;
use sugarsubstitute_contracts::execution::JobContext;

#[derive(Default)]
struct PartialInitialization(Fake);
impl NativePort for PartialInitialization {
    fn initialize(
        &self,
        root: &mut Root,
        _: &std::path::Path,
        _: &JobContext,
    ) -> Result<(), ClaimError> {
        if root.pool.is_none() {
            return Err(ClaimError::invalid("missing_actual_pool"));
        }
        Err(ClaimError::new(
            "initialize_before_key",
            std::io::Error::other("original-initialization-cause"),
        ))
    }
    fn resolve(&self, resolution: &mut Resolution, sid: &[u8]) -> Result<(), ClaimError> {
        self.0.resolve(resolution, sid)
    }
    fn acquire(
        &self,
        backend: &mut Backend,
        path: &std::path::Path,
        context: &JobContext,
    ) -> Result<(), ClaimError> {
        self.0.acquire(backend, path, context)
    }
    fn publish(
        &self,
        backend: &mut Backend,
        state: RecordState,
        process: u64,
        context: &JobContext,
    ) -> Result<(), ClaimError> {
        self.0.publish(backend, state, process, context)
    }
    fn release(&self, backend: &mut Backend, context: &JobContext) -> Result<(), ClaimError> {
        self.0.release(backend, context)
    }
    fn close(&self, root: &mut Root, context: &JobContext) -> Result<(), ClaimError> {
        self.0.close(root, context)
    }
}

#[test]
fn unbound_close_retires_failed_initialization_actual_pool_without_registry() -> TestResult {
    let mut h = Harness::new()?;
    h.claims.port = Arc::new(PartialInitialization::default());
    let spec = h.spec(crate::CLAIM_POOL_BYTES)?;
    h.claims.submit_initialize(&h.client, &h.scope, spec, PathBuf::from(r"E:\fixture"))?;
    h.drain()?;
    assert!(h.claims.owner.is_none());
    assert!(h.claims.registry.is_none());
    assert!(lock(&h.claims.root.value).pool.is_some());
    let cause = match h.claims.take_observation() {
        Some(ClaimObservation::Failed { cause, .. }) => cause,
        _ => return Err("Missing actual initialization failure".into()),
    };
    assert_eq!(cause.operation(), "initialize_before_key");
    h.claims.close_admission();
    let close = h.close_spec()?;
    h.claims.submit_close_unbound(&h.client, &h.scope, close)?;
    h.arm_memory_release(0)?;
    h.drain()?;
    h.wait_reserved(&h.scope, 0)?;
    assert!(h.claims.closed);
    assert!(lock(&h.claims.root.value).pool.is_none());
    assert!(matches!(h.claims.take_observation(), Some(ClaimObservation::NativeClaimsClosed)));
    assert!(std::error::Error::source(&cause).is_some());
    Ok(())
}

#[test]
fn unbound_close_failure_retains_actual_pool_and_original_cleanup_cutoff() -> TestResult {
    let mut h = Harness::new()?;
    let native = Arc::new(PartialInitialization::default());
    h.claims.port = native.clone();
    let init = h.spec(crate::CLAIM_POOL_BYTES)?;
    h.claims.submit_initialize(&h.client, &h.scope, init, PathBuf::from(r"E:\fixture"))?;
    h.drain()?;
    let original = h.claims.take_observation().ok_or("Missing initial failure")?;
    *lock(&native.0.failure) = Some(Stage::Close);
    h.claims.close_admission();
    let first = h.close_spec()?;
    h.claims.submit_close_unbound(&h.client, &h.scope, first)?;
    h.drain()?;
    assert!(!h.claims.closed);
    assert!(lock(&h.claims.root.value).pool.is_some());
    assert!(matches!(h.claims.take_observation(), Some(ClaimObservation::Failed { .. })));
    let second = h.close_spec()?;
    assert_eq!(first.deadline, second.deadline);
    h.claims.submit_close_unbound(&h.client, &h.scope, second)?;
    h.arm_memory_release(0)?;
    h.drain()?;
    h.wait_reserved(&h.scope, 0)?;
    assert!(h.claims.closed);
    assert!(matches!(original, ClaimObservation::Failed { .. }));
    assert_eq!(
        lock(&native.0.close_deadlines).as_slice(),
        &[first.deadline.ok_or("No cutoff")?, second.deadline.ok_or("No cutoff")?]
    );
    Ok(())
}

#[test]
fn unbound_close_rejects_acquired_coordinator_without_using_an_unrelated_registry() -> TestResult {
    let mut h = Harness::new()?;
    h.initialize()?;
    h.claims.close_admission();
    let spec = h.close_spec()?;
    assert!(matches!(
        h.claims.submit_close_unbound(&h.client, &h.scope, spec),
        Err(ClaimAdmissionError::Identity)
    ));
    assert!(!h.claims.closed);
    assert!(lock(&h.claims.root.value).pool.is_some());
    h.close(0)?;
    Ok(())
}

#[test]
fn unbound_close_after_rejected_initialization_has_no_false_key_or_pool() -> TestResult {
    let mut h = Harness::new()?;
    let native = Arc::new(PartialInitialization::default());
    h.claims.port = native.clone();
    let invalid = h.spec(crate::CLAIM_POOL_BYTES - 1)?;
    assert!(matches!(
        h.claims.submit_initialize(&h.client, &h.scope, invalid, PathBuf::from(r"E:\fixture")),
        Err(ClaimAdmissionError::Specification)
    ));
    assert!(h.claims.owner.is_none());
    assert!(h.claims.initialization_scope().is_none());
    assert!(lock(&h.claims.root.value).pool.is_none());
    h.claims.close_admission();
    let close = h.close_spec()?;
    h.claims.submit_close_unbound(&h.client, &h.scope, close)?;
    h.drain()?;
    assert!(h.claims.closed);
    assert_eq!(lock(&native.0.close_deadlines).len(), 1);
    Ok(())
}

#[test]
fn unbound_close_waits_for_actual_initialization_terminal_consumption() -> TestResult {
    let mut h = Harness::new()?;
    let native = Arc::new(PartialInitialization::default());
    h.claims.port = native.clone();
    let init = h.spec(crate::CLAIM_POOL_BYTES)?;
    h.claims.submit_initialize(&h.client, &h.scope, init, PathBuf::from(r"E:\fixture"))?;
    h.claims.close_admission();
    let first = h.close_spec()?;
    assert!(matches!(
        h.claims.submit_close_unbound(&h.client, &h.scope, first),
        Err(ClaimAdmissionError::Closed)
    ));
    assert!(lock(&native.0.close_deadlines).is_empty());
    h.drain()?;
    let close = h.close_spec()?;
    assert_eq!(first.deadline, close.deadline);
    h.claims.submit_close_unbound(&h.client, &h.scope, close)?;
    h.drain()?;
    assert!(h.claims.closed);
    Ok(())
}
