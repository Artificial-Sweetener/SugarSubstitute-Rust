// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep actual rejected counter data and final original custody across aliases and ACK.
use super::*;
use std::sync::atomic::AtomicBool;

struct CounterLease {
    calls: Arc<AtomicUsize>,
    on_sample: Option<Box<dyn FnOnce() + Send>>,
    memory: MemoryLease,
    data_dropped_while_claimed: Arc<AtomicBool>,
}
struct ObservedCause {
    memory: MemoryLease,
    data_dropped_while_claimed: Arc<AtomicBool>,
}
impl std::fmt::Debug for ObservedCause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ObservedCause")
    }
}
impl std::fmt::Display for ObservedCause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("actual synthetic counter failure")
    }
}
impl std::error::Error for ObservedCause {}
impl Drop for ObservedCause {
    fn drop(&mut self) {
        self.data_dropped_while_claimed.store(self.memory.claim(1).is_err(), Ordering::Release);
    }
}
impl EntryLease for CounterLease {
    fn sample_counter(&mut self) -> Result<(u64, u64), NativeObservationError> {
        self.calls.fetch_add(1, Ordering::AcqRel);
        if let Some(action) = self.on_sample.take() {
            action();
        }
        Err(NativeObservationError::new(
            "controlled_counter_failure",
            ObservedCause {
                memory: self.memory.clone(),
                data_dropped_while_claimed: self.data_dropped_while_claimed.clone(),
            },
        ))
    }
    fn prepare(
        &mut self,
        s: &[[Arc<Signal>; 3]],
        c: &JobContext,
        m: Moment,
    ) -> Result<(), EntryTransportError> {
        Lease.prepare(s, c, m)
    }
    fn connect(
        &mut self,
        i: usize,
        n: NativeIssue<'_>,
    ) -> Result<IssueDispatch<Progress>, EntryTransportError> {
        Lease.connect(i, n)
    }
    fn read(
        &mut self,
        i: usize,
        b: &mut [u8],
        n: NativeIssue<'_>,
    ) -> Result<IssueDispatch<Progress>, EntryTransportError> {
        Lease.read(i, b, n)
    }
    fn write(
        &mut self,
        i: usize,
        b: &[u8],
        n: NativeIssue<'_>,
    ) -> Result<IssueDispatch<Progress>, EntryTransportError> {
        Lease.write(i, b, n)
    }
    fn poll(&mut self, i: usize, r: Role) -> Result<Progress, EntryTransportError> {
        Lease.poll(i, r)
    }
    fn quiesce(&mut self, i: usize, r: Role) -> Result<(), EntryTransportError> {
        Lease.quiesce(i, r)
    }
    fn verify(&mut self, i: usize, m: bool) -> Result<KernelPeer, EntryTransportError> {
        Lease.verify(i, m)
    }
    fn nonce(&mut self) -> Result<[u8; 32], EntryTransportError> {
        Lease.nonce()
    }
    fn allow_foreground(&mut self, pid: u32) -> Result<(), NativeObservationError> {
        Lease.allow_foreground(pid)
    }
    fn retire_connection(&mut self, i: usize, r: bool) -> Result<bool, EntryTransportError> {
        Lease.retire_connection(i, r)
    }
    fn retire(&mut self) -> Result<bool, EntryTransportError> {
        Lease.retire()
    }
}
fn submit_counter(
    fixture: &mut Fixture,
    lease: CounterLease,
) -> TestResult<EntryTransportAdmissionError> {
    let input = *fixture.active.work.input.take().ok_or("Original complete input missing")?;
    let mut specification = spec(3, JobClass::NativeObservation, MIN_ENTRY_CLIENT_BYTES)?;
    specification.operation = Operation::NativeLifecycle;
    match reserve_client(
        &fixture.core,
        &fixture.client_scope,
        specification,
        input,
        Box::new(lease),
    ) {
        Err(error) => Ok(error),
        Ok(_) => Err("Failing counter admitted native client".into()),
    }
}
fn counter(
    fixture: &Fixture,
    calls: &Arc<AtomicUsize>,
    dropped: &Arc<AtomicBool>,
    action: Option<Box<dyn FnOnce() + Send>>,
) -> CounterLease {
    CounterLease {
        calls: calls.clone(),
        on_sample: action,
        memory: fixture.prepared.owner.original_context().memory().clone(),
        data_dropped_while_claimed: dropped.clone(),
    }
}

#[test]
fn destination_provider_revocation_vetoes_actual_counter_effect() -> TestResult {
    let mut fixture = Fixture::new()?;
    let core = fixture.core.clone();
    let original = fixture.original.id();
    let calls = Arc::new(AtomicUsize::new(0));
    let dropped = Arc::new(AtomicBool::new(false));
    let lease = counter(&fixture, &calls, &dropped, None);
    fixture.clock.on_next_current_thread_observation(Box::new(move || core.close_scope(original)));
    let error = submit_counter(&mut fixture, lease)?;
    assert!(matches!(error, EntryTransportAdmissionError::Preparation(_)));
    assert_eq!(calls.load(Ordering::Acquire), 0);
    assert!(!lock(&fixture.core.state).jobs.contains_key(&JobId::new(3)?));
    assert_eq!(fixture.client_scope.snapshot().reserved_bytes, MIN_ENTRY_CLIENT_BYTES);
    Ok(())
}

#[test]
fn failed_counter_and_actual_post_cancel_keep_original_pair_through_ack_and_last_clone()
-> TestResult {
    let mut fixture = Fixture::new()?;
    let core = fixture.core.clone();
    let original = fixture.original.id();
    let calls = Arc::new(AtomicUsize::new(0));
    let dropped = Arc::new(AtomicBool::new(false));
    let lease =
        counter(&fixture, &calls, &dropped, Some(Box::new(move || core.cancel_scope(original))));
    let error = submit_counter(&mut fixture, lease)?;
    let EntryTransportAdmissionError::Prepared(error) = error else {
        return Err("Lost complete prepared counter error".into());
    };
    let EntryTransportError::PreparedRetirement(ref retained) = error else {
        return Err("Counter failure was unpaired".into());
    };
    assert!(
        matches!(retained.original(),Some(cause) if cause.kind()==crate::EntryPreparedCauseKind::Clock && cause.native_stage()==Some("controlled_counter_failure"))
    );
    assert!(matches!(
        retained.cleanup_causes().next().and_then(crate::EntryPreparedCause::preparation_cause),
        Some(EntryPreparationInputError::Control(
            crate::entry_preparation::EntryPreparationControlCause::Cancelled
        ))
    ));
    assert_eq!(calls.load(Ordering::Acquire), 1);
    let core = fixture.core.clone();
    let scope = fixture.original.id();
    let cloned = error.clone();
    drop(fixture);
    assert_eq!(core.scope_snapshot(scope).reserved_bytes, 4 * 1024 * 1024);
    assert!(!dropped.load(Ordering::Acquire));
    drop(error);
    assert_eq!(core.scope_snapshot(scope).reserved_bytes, 4 * 1024 * 1024);
    drop(cloned);
    assert!(dropped.load(Ordering::Acquire));
    assert_eq!(core.scope_snapshot(scope).reserved_bytes, 0);
    Ok(())
}

#[test]
fn bounded_later_causes_preserve_omitted_actual_fatal_and_panic_facts_without_self_cycle()
-> TestResult {
    let mut fixture = Fixture::new()?;
    fixture.retain_source();
    fixture.active.source_pending =
        Some(EntrySourceOutcome::Launch(Err(EntryTransportError::Malformed)));
    fixture.core.cancel_scope(fixture.original.id());
    let mut error = fixture
        .active
        .complete_source_step(&fixture.core)
        .err()
        .ok_or("Original failure missing")?;
    let same = error.clone();
    error.retain_cleanup(same);
    let EntryTransportError::PreparedRetirement(ref retained) = error else {
        return Err("Pair missing".into());
    };
    assert_eq!(retained.cleanup_causes().count(), 1);
    for _ in 0..7 {
        error.retain_cleanup(EntryTransportError::Closed);
    }
    let caught = std::panic::catch_unwind(|| {
        std::panic::resume_unwind(Box::new("caught actual boundary unwind"))
    })
    .err()
    .ok_or("Boundary did not unwind")?;
    let panic = crate::task::panic_cause(caught);
    let mut native = NativeObservationError::new(
        "late_identity_restore",
        std::io::Error::other("actual controlled failure"),
    );
    native.retain_panic(&panic);
    error.retain_cleanup(EntryTransportError::Native { cause: native, identity_unrestored: true });
    let EntryTransportError::PreparedRetirement(ref retained) = error else {
        return Err("Pair replaced".into());
    };
    assert_eq!(retained.cleanup_causes().count(), 8);
    assert_eq!(retained.omitted_cleanup(), 1);
    assert!(error.requires_process_shutdown());
    assert!(error.contains_owner_panic());
    assert_eq!(
        retained.original().map(crate::EntryPreparedCause::kind),
        Some(crate::EntryPreparedCauseKind::Malformed)
    );
    Ok(())
}
