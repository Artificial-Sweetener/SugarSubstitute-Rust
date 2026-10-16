// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Diagnostic panics exercise actual admitted charge with native-free owners.
use super::retain_before_diagnostics;
use std::{
    num::NonZeroUsize,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use sugarsubstitute_contracts::execution::*;
use sugarsubstitute_execution::ExecutionOwner;

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
        let mut count = self.count.lock().map_err(|_| {
            ExecutionError::new(
                Operation::NativeLifecycle,
                Box::new(std::io::Error::other("Poisoned test wake")),
            )
        })?;
        *count += 1;
        self.changed.notify_all();
        Ok(())
    }
}
struct RetainedOwner {
    charge: Arc<Mutex<Option<MemoryClaim>>>,
    dropped: Arc<AtomicUsize>,
}
impl Drop for RetainedOwner {
    fn drop(&mut self) {
        self.dropped.fetch_add(1, Ordering::AcqRel);
    }
}

#[test]
fn diagnostic_panic_cannot_drop_irrevocably_retained_owner_or_actual_charge()
-> Result<(), Box<dyn std::error::Error>> {
    let wake = Arc::new(WakeBoundary::default());
    let owner = ExecutionOwner::new(Arc::new(ClockBoundary), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let spec = JobSpec {
        id: JobId::new(1)?,
        operation: Operation::NativeLifecycle,
        class: JobClass::InfrastructureLifecycle,
        deadline: Some(Moment::from_nanos(1)),
        reservation_bytes: 4096,
        replacement: None,
    };
    let mut job = client.submit_blocking(&scope, spec, |context| context.memory().claim(4096))?;
    let completion = loop {
        let count = *wake.count.lock().map_err(|_| "Poisoned test wake")?;
        if let Some(completion) = job.try_take_completion() {
            break completion;
        }
        client.drain_completions(NonZeroUsize::MIN)?;
        if let Some(completion) = job.try_take_completion() {
            break completion;
        }
        let (observed, timeout) = wake
            .changed
            .wait_timeout_while(
                wake.count.lock().map_err(|_| "Poisoned test wake")?,
                Duration::from_secs(10),
                |observed| *observed == count,
            )
            .map_err(|_| "Poisoned test wake")?;
        if timeout.timed_out() && *observed == count {
            return Err("Actual job did not complete".into());
        }
    };
    let JobOutcome::Success(memory) = completion.outcome else {
        return Err("Actual charge job failed".into());
    };
    drop(completion.memory);
    let charge = Arc::new(Mutex::new(Some(memory)));
    let dropped = Arc::new(AtomicUsize::new(0));
    let durable = Box::new(RetainedOwner { charge: charge.clone(), dropped: dropped.clone() });
    let panic = catch_unwind(AssertUnwindSafe(|| {
        retain_before_diagnostics(durable, |retained| {
            assert!(retained.charge.lock().is_ok_and(|memory| memory.is_some()));
            std::panic::resume_unwind(Box::new("Injected diagnostic panic"));
        })
    }));
    assert!(panic.is_err());
    assert_eq!(dropped.load(Ordering::Acquire), 0);
    assert_eq!(scope.snapshot().reserved_bytes, 4096);
    assert!(
        charge
            .lock()
            .is_ok_and(|memory| memory.as_ref().is_some_and(|memory| memory.bytes() == 4096))
    );
    // Only the fake charge is explicitly settled after proving retention. The tiny
    // process-lifetime owner stays retained exactly as the production fallback does.
    charge.lock().map_err(|_| "Poisoned test charge")?.take();
    drop(job);
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    assert_eq!(dropped.load(Ordering::Acquire), 0);
    drop(scope);
    Ok(())
}

#[test]
fn actual_original_partitions_and_trusted_bundle_exactly_fit_four_mebibytes()
-> Result<(), Box<dyn std::error::Error>> {
    use sugarsubstitute_contracts::lifecycle::{CoordinatorEpoch, CoordinatorKey, NamespaceId};
    let wake = Arc::new(WakeBoundary::default());
    let owner = ExecutionOwner::new(Arc::new(ClockBoundary), wake.clone())?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let request = JobSpec {
        id: JobId::new(1)?,
        operation: Operation::NativeLifecycle,
        class: JobClass::InfrastructureLifecycle,
        deadline: Some(Moment::from_nanos(1)),
        reservation_bytes: crate::input::STORAGE_BYTES as u64,
        replacement: None,
    };
    let key = CoordinatorKey {
        namespace: NamespaceId::new([1; 32]),
        epoch: CoordinatorEpoch::new([2; 16])?,
    };
    let capture_client = client.clone();
    let mut job = client.submit_blocking(&scope, request, move |context| {
        let admission =
            capture_client.capture_bundle_admission(&context).map_err(|cause| cause.to_string())?;
        let memory = context.memory();
        let source = memory
            .claim(crate::input::SOURCE_STORAGE_BYTES as u64)
            .map_err(|cause| cause.to_string())?;
        let ledger = crate::capture_endpoint::Attempts::new(key, source)
            .map_err(|cause| cause.to_string())?;
        let capture =
            super::Capture::new(admission, capture_client).map_err(|cause| cause.to_string())?;
        let fields = capture.fields.as_ref().ok_or("Missing actual native owner")?;
        assert_eq!(fields._storage.bytes(), 966_656);
        assert_eq!(fields.pin_claim.as_ref().map(MemoryClaim::bytes), Some(65_536));
        let rejected =
            memory.claim(1).err().ok_or("Actual owners undercharged live allocations")?;
        assert_eq!(rejected.available_bytes, 0);
        assert!(fields.pins.is_empty());
        assert!(fields.storage.as_ref().is_none_or(|storage| storage.is_retired()));
        assert!(
            size_of::<super::Fields>()
                + size_of::<super::Capture>()
                + size_of::<crate::capture::Shared>()
                + 2 * size_of::<usize>()
                + 64
                < 16_384
        );
        // Empty allocation-only native owners have no unresolved kernel effects.
        // Their real Drop releases exactly the three distinct guarded claims.
        drop(capture);
        let remaining = memory
            .claim(crate::input::STORAGE_BYTES as u64 - 16_384)
            .map_err(|cause| cause.to_string())?;
        assert!(memory.claim(1).is_err());
        drop(remaining);
        Ok::<_, String>(ledger)
    })?;
    let completion = loop {
        let previous = *wake.count.lock().map_err(|_| "Poisoned test wake")?;
        if let Some(completion) = job.try_take_completion() {
            break completion;
        }
        client.drain_completions(NonZeroUsize::MIN)?;
        if let Some(completion) = job.try_take_completion() {
            break completion;
        }
        let (count, timeout) = wake
            .changed
            .wait_timeout_while(
                wake.count.lock().map_err(|_| "Poisoned test wake")?,
                Duration::from_secs(10),
                |count| *count == previous,
            )
            .map_err(|_| "Poisoned test wake")?;
        if timeout.timed_out() && *count == previous {
            return Err("Actual allocation job did not finish".into());
        }
    };
    let JobOutcome::Success(ledger) = completion.outcome else {
        return Err("Actual allocation job failed".into());
    };
    drop(completion.memory);
    assert_eq!(scope.snapshot().reserved_bytes, crate::input::STORAGE_BYTES as u64);
    drop(ledger);
    drop(job);
    drop(owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
