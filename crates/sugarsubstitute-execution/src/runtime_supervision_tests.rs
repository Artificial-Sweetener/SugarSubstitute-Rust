// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preserve native deadline wake independently of cancellable timer selection.

use super::*;
use crate::{
    native::{self, NativeCorrelation, NativeObservationError, ProcessExit, ProcessLease, Signal},
    runtime::ExecutionOwner,
    runtime_test_support::{Gate, Rig, TestResult, Wake, spec},
};
use std::sync::atomic::{AtomicUsize, Ordering};
use sugarsubstitute_contracts::execution::*;

#[path = "runtime_supervision_test_clock.rs"]
mod clock;
use clock::{ControlledClock, ReleaseOnDrop, TimerBoundary};

struct ProcessBoundary {
    started: Gate,
    retired: AtomicUsize,
}
struct Input {
    process: Arc<ProcessBoundary>,
    timer: Arc<TimerBoundary>,
}
impl ProcessLease for Input {
    fn start(
        &mut self,
        _: NativeCorrelation,
        _: Arc<Signal>,
    ) -> Result<(), NativeObservationError> {
        self.process.started.open();
        self.timer.native_start_release.wait().map_err(|cause| {
            NativeObservationError::new(
                "held_registration",
                std::io::Error::other(cause.to_string()),
            )
        })?;
        self.timer.native_thread.set(std::thread::current().id()).map_err(|_| {
            NativeObservationError::new(
                "registration_thread",
                std::io::Error::other("Thread installed twice"),
            )
        })?;
        Ok(())
    }
    fn read_exit(&mut self) -> Result<ProcessExit, NativeObservationError> {
        Err(NativeObservationError::new(
            "unexpected_exit",
            std::io::Error::other("Unsignalled fake process"),
        ))
    }
    fn retire(&mut self) -> Result<(), NativeObservationError> {
        self.process.retired.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }
}
fn native_scope(rig: &Rig) -> TestResult<Scope> {
    Ok(rig.client.core.create_scope_kind(
        ScopeSpec { id: ScopeId::new(2)?, deadline: None },
        Some(&rig.scope),
        true,
    )?)
}
fn native_spec(id: u64, deadline: Moment) -> TestResult<JobSpec> {
    let mut spec = spec(id, JobClass::NativeObservation, 4096)?;
    spec.operation = Operation::NativeLifecycle;
    spec.deadline = Some(deadline);
    Ok(spec)
}

#[test]
fn expired_unbound_native_deadlines_wake_without_becoming_immediate_timers() -> TestResult {
    let rig = Rig::new()?;
    let scope = native_scope(&rig)?;
    let first = native::reserve(
        &rig.client.core,
        &scope,
        native_spec(1, Moment::from_nanos(100))?,
        NativeCorrelation::new(1)?,
    )?;
    let later = native::reserve(
        &rig.client.core,
        &scope,
        native_spec(2, Moment::from_nanos(200))?,
        NativeCorrelation::new(2)?,
    )?;
    {
        let state = lock(&rig.client.core.state);
        assert_eq!(
            deadlines(&state, Moment::from_nanos(0)),
            (Some(Moment::from_nanos(100)), false)
        );
        assert_eq!(
            deadlines(&state, Moment::from_nanos(100)),
            (Some(Moment::from_nanos(200)), true)
        );
        assert_eq!(deadlines(&state, Moment::from_nanos(200)), (None, true));
    }
    // Expiry does not consume unbound ownership or its capacity. Scope cancel
    // similarly retains it until the permit is explicitly consumed or dropped.
    rig.clock.advance(200);
    scope.cancel();
    assert_eq!(scope.snapshot().queued, 2);
    assert_eq!(scope.snapshot().reserved_bytes, 8192);
    drop(first);
    drop(later);
    drop(rig.owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn notification_discarded_timer_still_retires_native_at_fresh_expired_scan() -> TestResult {
    let timer = TimerBoundary::new();
    let wake = Arc::new(Wake::default());
    let owner = ExecutionOwner::with_cpu_workers(
        Arc::new(ControlledClock(timer.clone())),
        wake.clone(),
        2,
    )?;
    let client = owner.client();
    let scope = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let rig = Rig { owner, client, clock: timer.clock.clone(), wake, scope };
    let release = ReleaseOnDrop(timer.clone());
    let scope = native_scope(&rig)?;
    let deadline = Moment::from_nanos(100);
    let boundary =
        Arc::new(ProcessBoundary { started: Gate::default(), retired: AtomicUsize::new(0) });
    let mut job = native::reserve(
        &rig.client.core,
        &scope,
        native_spec(1, deadline)?,
        NativeCorrelation::new(1)?,
    )?
    .bind_input(Box::new(Input { process: boundary.clone(), timer: timer.clone() }));
    boundary.started.wait()?;
    rig.clock.wait_until_registered(deadline)?;
    timer.armed.store(true, Ordering::Release);
    // At time 0 the timer is still pending, so only the notification branch
    // can select. Its disposal barrier prevents the next fresh clock scan.
    rig.client.core.changed();
    timer.disposing.wait()?;
    // Registration is held until the earlier native unpark token exists. The
    // third native clock sample acknowledges a full zero-time park/loop pass,
    // consuming that token before this test advances the monotonic epoch.
    timer.native_start_release.open();
    timer.native_zero_observed.wait()?;
    assert_eq!(ControlledClock(timer.clone()).now(), Moment::from_nanos(0));
    rig.clock.advance(100);
    {
        let state = lock(&rig.client.core.state);
        assert_eq!(deadlines(&state, deadline), (None, true));
    }
    timer.release.open();
    // No application activity/wake is issued after advancement. The actual
    // owner must independently observe expiry and retire the unsignalled lease.
    let completion = rig.completion(&mut job)?;
    assert!(matches!(completion.outcome, JobOutcome::TimedOut));
    assert_eq!(completion.spec.id, JobId::new(1)?);
    assert_eq!(completion.scope_id, scope.id());
    assert_eq!(boundary.retired.load(Ordering::Acquire), 1);
    assert_eq!(rig.clock.expired_sleep_calls.load(Ordering::Acquire), 0);
    assert!(lock(&timer.failure).is_none());
    drop(completion);
    drop(rig.owner);
    drop(release);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}
