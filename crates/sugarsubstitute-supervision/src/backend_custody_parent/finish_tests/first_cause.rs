// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain a first boundary failure through the actual parent funded by a genuine pipe session.
use super::*;
use std::{
    error::Error,
    num::NonZeroUsize,
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};
use sugarsubstitute_contracts::execution::*;
use sugarsubstitute_execution::{ExecutionOwner, windows_lifecycle_pipe::MIN_LIFECYCLE_PIPE_BYTES};
struct Clock;
impl sugarsubstitute_contracts::execution::Clock for Clock {
    fn now(&self) -> Moment {
        Moment::from_nanos(0)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
#[derive(Default)]
struct Wake {
    revision: Mutex<u64>,
    changed: Condvar,
}
impl CompletionWake for Wake {
    fn wake(&self) -> Result<(), ExecutionError> {
        *self.revision.lock().unwrap_or_else(std::sync::PoisonError::into_inner) += 1;
        self.changed.notify_all();
        Ok(())
    }
}
#[test]
fn actual_parent_first_cause_survives_finish_and_native_pipe_cancellation() -> TestResult {
    let wake = Arc::new(Wake::default());
    let executor = ExecutionOwner::new(Arc::new(Clock), wake.clone())?;
    let client = executor.client();
    let scope =
        client.create_native_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let until = Moment::from_nanos(30_000_000_000);
    let mut session = Some(client.reserve_backend_custody_pipe_server(
        &scope,
        JobSpec {
            id: JobId::new(1)?,
            operation: Operation::NativeLifecycle,
            class: JobClass::NativeObservation,
            reservation_bytes: MIN_LIFECYCLE_PIPE_BYTES,
            deadline: None,
            replacement: None,
        },
        key()?,
        until,
    )?);
    let parent = BackendCustodyParent::new(&mut session)?;
    assert!(session.is_none());
    assert_eq!(parent.finish_status(), crate::BackendCustodyFinishStatus::Unissued);
    let loaned_terminal = parent.access(|_| {
        assert_eq!(parent.finish_status(), crate::BackendCustodyFinishStatus::Unavailable);
        Ok(parent.take_terminal())
    })??;
    assert!(loaned_terminal.is_none());
    assert!(parent.cached_failure().is_none());
    assert_eq!(parent.finish_status(), crate::BackendCustodyFinishStatus::Unissued);
    let original = NativeObservationError::new(
        "original_external_boundary",
        std::io::Error::other("Bounded original failure"),
    );
    let pointer = std::ptr::from_ref(original.source().ok_or("Original source absent")?);
    let rejected = parent.access::<()>(|_| Err(original)).err().ok_or("Boundary failure lost")?;
    let held = rejected
        .source()
        .and_then(|cause| cause.downcast_ref::<NativeObservationError>())
        .ok_or("Original type lost")?;
    assert_eq!(std::ptr::from_ref(held.source().ok_or("Held source absent")?), pointer);
    for _ in 0..2 {
        let rejected =
            parent.advance_finish(&client, until).err().ok_or("Failed parent accepted Finish")?;
        let held = rejected
            .source()
            .and_then(|cause| cause.downcast_ref::<NativeObservationError>())
            .ok_or("First type lost")?;
        assert_eq!(held.stage(), "original_external_boundary");
        assert_eq!(std::ptr::from_ref(held.source().ok_or("First source absent")?), pointer);
    }
    loop {
        let observed = *wake.revision.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        client.drain_completions(NonZeroUsize::MIN)?;
        if let Some(terminal) = parent.take_terminal()? {
            assert!(matches!(terminal.outcome, JobOutcome::Cancelled));
            break;
        }
        let guard = wake.revision.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let (guard, timeout) = wake
            .changed
            .wait_timeout_while(guard, Duration::from_secs(10), |current| *current == observed)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        drop(guard);
        if timeout.timed_out() {
            return Err("Original pipe cancellation did not retire".into());
        }
    }
    drop(rejected);
    drop(parent);
    drop(executor);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
