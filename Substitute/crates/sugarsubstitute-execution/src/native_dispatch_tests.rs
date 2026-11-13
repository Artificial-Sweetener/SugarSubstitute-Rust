// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Reproduce admission after a native iteration sample through governed clocks and leases.

use super::*;
use crate::{
    native::{NativeCorrelation, NativeObservationError, ProcessExit, reserve},
    runtime_test_support::{ClockPort, Gate, ManualClock, Rig, TestResult, Wake, spec},
};
use std::{
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    time::Duration,
};
use sugarsubstitute_contracts::execution::*;

/// Suspend a real clock observation before the native worker can dequeue later admission.
struct DispatchClock {
    manual: Arc<ManualClock>,
    sampled: Gate,
    release: Gate,
    hold_sample: AtomicBool,
    panic_dispatch: AtomicBool,
    creator: std::thread::ThreadId,
    creator_reads: AtomicUsize,
}
impl Clock for DispatchClock {
    fn now(&self) -> Moment {
        if std::thread::current().id() == self.creator {
            self.creator_reads.fetch_add(1, Ordering::AcqRel);
        }
        let observed = ClockPort(self.manual.clone()).now();
        if std::thread::current().name() == Some("sugarsubstitute-native-retirement") {
            if self.hold_sample.swap(false, Ordering::AcqRel) {
                self.sampled.open();
                if let Err(cause) = self.release.wait() {
                    std::panic::resume_unwind(Box::new(cause.to_string()));
                }
            } else if self.panic_dispatch.swap(false, Ordering::AcqRel) {
                std::panic::resume_unwind(Box::new("post-dequeue clock failure".to_owned()));
            }
        }
        observed
    }
    fn sleep_until(&self, deadline: Moment) -> WaitFuture {
        ClockPort(self.manual.clone()).sleep_until(deadline)
    }
}

/// Observe registration and actual teardown separately from cancellation or deadline metadata.
struct Boundary {
    starts: AtomicUsize,
    retire_entered: Gate,
    retire_release: Gate,
    closes: AtomicUsize,
    drops: AtomicUsize,
    hold_retirement: bool,
}
struct Lease(Arc<Boundary>);
impl ProcessLease for Lease {
    fn start(
        &mut self,
        _: NativeCorrelation,
        signal: Arc<Signal>,
    ) -> Result<(), NativeObservationError> {
        self.0.starts.fetch_add(1, Ordering::AcqRel);
        signal.complete();
        Ok(())
    }
    fn read_exit(&mut self) -> Result<ProcessExit, NativeObservationError> {
        Ok(ProcessExit {
            correlation: NativeCorrelation::new(1)
                .map_err(|cause| NativeObservationError::new("fixture_correlation", cause))?,
            process_id: 17,
            creation_time_100ns: 23,
            exit_code: 0,
        })
    }
    fn retire(&mut self) -> Result<(), NativeObservationError> {
        self.0.retire_entered.open();
        if self.0.hold_retirement {
            self.0.retire_release.wait().map_err(|cause| {
                NativeObservationError::new(
                    "fixture_retirement",
                    std::io::Error::other(cause.to_string()),
                )
            })?;
        }
        self.0.closes.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        self.0.drops.fetch_add(1, Ordering::AcqRel);
    }
}

/// Keep gates open during assertion unwinding so the governed owner can still join.
struct Fixture {
    rig: Rig,
    clock: Arc<DispatchClock>,
    boundary: Arc<Boundary>,
    native_scope: Scope,
}
impl Fixture {
    /// Establish the paused iteration before admitting any native job.
    fn new(hold_retirement: bool) -> TestResult<Self> {
        let manual = Arc::new(ManualClock::default());
        let clock = Arc::new(DispatchClock {
            manual: manual.clone(),
            sampled: Gate::default(),
            release: Gate::default(),
            hold_sample: AtomicBool::new(true),
            panic_dispatch: AtomicBool::new(false),
            creator: std::thread::current().id(),
            creator_reads: AtomicUsize::new(0),
        });
        let boundary = Arc::new(Boundary {
            starts: AtomicUsize::new(0),
            retire_entered: Gate::default(),
            retire_release: Gate::default(),
            closes: AtomicUsize::new(0),
            drops: AtomicUsize::new(0),
            hold_retirement,
        });
        let wake = Arc::new(Wake::default());
        let owner = crate::ExecutionOwner::with_cpu_workers(clock.clone(), wake.clone(), 2)?;
        let client = owner.client();
        let scope =
            client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
        let native_scope = client.core.create_scope_kind(
            ScopeSpec { id: ScopeId::new(2)?, deadline: None },
            Some(&scope),
            true,
        )?;
        let fixture = Self {
            rig: Rig { owner, client, clock: manual, wake, scope },
            clock,
            boundary,
            native_scope,
        };
        fixture.clock.sampled.wait()?;
        Ok(fixture)
    }
    /// Admit at 10ms while the native worker still retains its earlier 0ms sample.
    fn bind_later(
        &self,
        deadline: Option<Moment>,
    ) -> TestResult<OwnedJob<ProcessExit, NativeObservationError>> {
        self.rig.clock.advance(10_000_000);
        let mut specification = spec(1, JobClass::NativeObservation, 4096)?;
        specification.operation = Operation::NativeLifecycle;
        specification.deadline = deadline;
        Ok(reserve(
            &self.rig.client.core,
            &self.native_scope,
            specification,
            NativeCorrelation::new(1)?,
        )?
        .bind_input(Box::new(Lease(self.boundary.clone()))))
    }
    /// Move to 20ms before releasing the already sampled native iteration.
    fn resume_at_twenty_ms(&self) {
        self.rig.clock.advance(10_000_000);
        self.clock.release.open();
    }
    /// Consume the exact typed job despite separately reported fatal diagnostics.
    fn terminal_with_failures(
        &self,
        job: &mut OwnedJob<ProcessExit, NativeObservationError>,
    ) -> TestResult<(JobCompletion<ProcessExit, NativeObservationError>, Vec<ExecutionError>)> {
        let mut failures = Vec::new();
        loop {
            let observed = self.rig.wake.count();
            if let Some(completion) = job.try_take_completion() {
                return Ok((completion, failures));
            }
            if let Err(cause) = self.rig.client.drain_completions(std::num::NonZeroUsize::MIN) {
                failures.push(cause);
            }
            if let Some(completion) = job.try_take_completion() {
                return Ok((completion, failures));
            }
            self.rig.wake.wait_after(observed)?;
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.clock.release.open();
        self.boundary.retire_release.open();
    }
}

/// Queue timing must use a sample taken after the exact later job was removed.
#[test]
fn later_native_admission_gets_a_causal_dispatch_sample() -> TestResult {
    let fixture = Fixture::new(false)?;
    let mut job = fixture.bind_later(None)?;
    fixture.resume_at_twenty_ms();
    let completion = fixture.rig.completion(&mut job)?;
    assert!(matches!(completion.outcome, JobOutcome::Success(_)));
    assert_eq!(completion.queue_time, Duration::from_millis(10));
    assert_eq!(completion.execution_time, Duration::ZERO);
    assert_eq!(fixture.boundary.starts.load(Ordering::Acquire), 1);
    assert_eq!(fixture.boundary.closes.load(Ordering::Acquire), 1);
    assert_eq!(fixture.boundary.drops.load(Ordering::Acquire), 1);
    Ok(())
}

/// Expiry between the iteration sample and admission must forbid native registration.
#[test]
fn original_deadline_expired_before_dispatch_never_starts_a_native_lease() -> TestResult {
    let fixture = Fixture::new(false)?;
    let mut job = fixture.bind_later(Some(Moment::from_nanos(15_000_000)))?;
    fixture.resume_at_twenty_ms();
    let completion = fixture.rig.completion(&mut job)?;
    assert!(matches!(completion.outcome, JobOutcome::TimedOut));
    assert_eq!(completion.spec.deadline, Some(Moment::from_nanos(15_000_000)));
    assert_eq!(fixture.boundary.starts.load(Ordering::Acquire), 0);
    assert_eq!(fixture.boundary.closes.load(Ordering::Acquire), 1);
    assert_eq!(fixture.boundary.drops.load(Ordering::Acquire), 1);
    Ok(())
}

/// A new dispatch clock failure retains the dequeued lease until actual retirement.
#[test]
fn post_dequeue_clock_panic_retains_the_unstarted_native_lease_until_retirement() -> TestResult {
    let fixture = Fixture::new(true)?;
    let mut job = fixture.bind_later(None)?;
    fixture.clock.panic_dispatch.store(true, Ordering::Release);
    fixture.resume_at_twenty_ms();
    fixture.boundary.retire_entered.wait()?;
    let starts = fixture.boundary.starts.load(Ordering::Acquire);
    let closes = fixture.boundary.closes.load(Ordering::Acquire);
    let drops = fixture.boundary.drops.load(Ordering::Acquire);
    let pending = job.try_take_completion().is_none();
    let retained = fixture.native_scope.snapshot().reserved_bytes;
    let reads = fixture.clock.creator_reads.load(Ordering::Acquire);
    let clock_failed = fixture.rig.client.now().is_err();
    let reads_after_metadata = fixture.clock.creator_reads.load(Ordering::Acquire);
    let retained_now = fixture.rig.client.core.clock.retirement_now();
    let mut rejected = spec(2, JobClass::NativeObservation, 4096)?;
    rejected.operation = Operation::NativeLifecycle;
    let admission_closed = matches!(
        reserve(
            &fixture.rig.client.core,
            &fixture.native_scope,
            rejected,
            NativeCorrelation::new(2)?
        ),
        Err(AdmissionError::ShuttingDown)
    );
    fixture.boundary.retire_release.open();
    let (completion, _diagnostics) = fixture.terminal_with_failures(&mut job)?;
    assert_eq!(starts, 0);
    assert_eq!(closes, 0);
    assert_eq!(drops, 0);
    assert!(pending);
    assert_eq!(retained, 4096);
    assert!(clock_failed);
    assert_eq!(reads_after_metadata, reads);
    assert!(admission_closed);
    let JobOutcome::Panicked(cause) = completion.outcome else {
        return Err("Dispatch panic was replaced by another outcome".into());
    };
    assert_eq!(cause.as_str(), "post-dequeue clock failure");
    assert_eq!(
        completion.queue_time,
        retained_now.checked_duration_since(Moment::from_nanos(10_000_000))?
    );
    assert_eq!(completion.execution_time, Duration::ZERO);
    assert_eq!(fixture.boundary.closes.load(Ordering::Acquire), 1);
    assert_eq!(fixture.boundary.drops.load(Ordering::Acquire), 1);
    Ok(())
}
