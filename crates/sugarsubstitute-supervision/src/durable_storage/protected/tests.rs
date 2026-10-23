// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise actual admitted memory and pre-native context rejection without native calls.
use super::*;
use std::{
    num::NonZeroUsize,
    sync::{Condvar, Mutex},
    time::Duration,
};
use sugarsubstitute_contracts::execution::*;
use sugarsubstitute_execution::ExecutionOwner;
#[path = "capture_read/tests.rs"]
mod capture_read;
type TestResult = Result<(), Box<dyn std::error::Error>>;
struct FixedClock;
impl Clock for FixedClock {
    fn now(&self) -> Moment {
        Moment::from_nanos(17)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
#[derive(Default)]
struct Wake {
    count: Mutex<u64>,
    changed: Condvar,
}
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        *self.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner) += 1;
        self.changed.notify_all();
        Ok(())
    }
}
fn admitted(
    work: impl FnOnce(JobContext) -> Result<(), ClaimError> + Send + 'static,
) -> TestResult {
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::new(Arc::new(FixedClock), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let mut job = client.submit_blocking(
        &scope,
        JobSpec {
            id: JobId::new(1)?,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(Moment::from_nanos(100)),
            reservation_bytes: STORAGE_BYTES,
            replacement: None,
        },
        work,
    )?;
    let completion = loop {
        let old = *wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(value) = job.try_take_completion() {
            break value;
        }
        client.drain_completions(NonZeroUsize::MIN)?;
        if let Some(value) = job.try_take_completion() {
            break value;
        }
        let (count, timeout) = wake
            .changed
            .wait_timeout_while(
                wake.count.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
                Duration::from_secs(10),
                |count| *count == old,
            )
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        if timeout.timed_out() && *count == old {
            return Err("Protected storage CPU owner deadline expired".into());
        }
    };
    match completion.outcome {
        JobOutcome::Success(()) => (),
        other => return Err(format!("Protected storage test job failed: {other:?}").into()),
    }
    drop(completion.memory);
    drop(job);
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
#[test]
fn zero_and_undersized_actual_claims_reject_before_durable_allocation() -> TestResult {
    admitted(|context| {
        for bytes in [0, STORAGE_BYTES - 1] {
            let claim =
                context.memory().claim(bytes).map_err(|e| ClaimError::new("test_claim", e))?;
            assert!(ProtectedStorageRoot::new(claim).is_err());
        }
        let claim =
            context.memory().claim(STORAGE_BYTES).map_err(|e| ClaimError::new("test_claim", e))?;
        let proof = ProtectedStorageRoot::new(claim)?;
        assert!(proof.is_retired());
        assert!(proof.generation().is_none());
        drop(proof);
        Ok(())
    })
}
#[test]
fn invalid_root_rejection_has_no_native_effects_and_checked_retirement_releases_charge()
-> TestResult {
    admitted(|context| {
        let claim =
            context.memory().claim(STORAGE_BYTES).map_err(|e| ClaimError::new("test_claim", e))?;
        let mut proof = ProtectedStorageRoot::new(claim)?;
        // Invalid lexical input rejects before token/path queries in the real owner.
        let invalid = Path::new("relative/private-root");
        assert!(proof.resolve(invalid, &context).is_err());
        assert!(proof.state.as_ref().is_some_and(|state| state.handles.is_empty()));
        assert!(proof.generation().is_none());
        use sugarsubstitute_contracts::lifecycle::{CoordinatorEpoch, NamespaceId};
        let key = CoordinatorKey {
            namespace: NamespaceId::new([1; 32]),
            epoch: CoordinatorEpoch::new([2; 16]).map_err(|e| ClaimError::new("test_epoch", e))?,
        };
        assert!(proof.create_capture_generation(key, &context).is_err());
        proof.retire(&context)?;
        assert!(proof.is_retired());
        Ok(())
    })
}

#[path = "account/diagnostic_root/tests.rs"]
mod diagnostic_root;
