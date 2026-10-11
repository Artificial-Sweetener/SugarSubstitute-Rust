// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Exercise admitted process lifetimes through the actual owner with controlled kernel boundaries.

use super::*;
use crate::runtime_test_support::{Gate, Rig, TestResult, spec};
use std::sync::atomic::{AtomicI32, AtomicU64, AtomicUsize};
use sugarsubstitute_contracts::execution::*;

struct Boundary {
    started: Gate,
    retiring: Gate,
    release: Gate,
    signal: Mutex<Option<Arc<Signal>>>,
    starts: AtomicUsize,
    closes: AtomicUsize,
    retire_failures: AtomicUsize,
    panic_start: AtomicBool,
    panic_read: AtomicBool,
    panic_retire: AtomicBool,
    drops: AtomicUsize,
    correlation: AtomicU64,
    retire_error: AtomicI32,
    second_error: Gate,
    complete_in_start: AtomicBool,
    hold_start: AtomicBool,
    start_release: Gate,
    start_failure: bool,
    hold_retirement: bool,
}
impl Boundary {
    fn new(hold_retirement: bool, start_failure: bool) -> Arc<Self> {
        Arc::new(Self {
            started: Gate::default(),
            retiring: Gate::default(),
            release: Gate::default(),
            signal: Mutex::new(None),
            starts: AtomicUsize::new(0),
            closes: AtomicUsize::new(0),
            retire_failures: AtomicUsize::new(0),
            panic_start: AtomicBool::new(false),
            panic_read: AtomicBool::new(false),
            panic_retire: AtomicBool::new(false),
            drops: AtomicUsize::new(0),
            correlation: AtomicU64::new(0),
            retire_error: AtomicI32::new(6),
            second_error: Gate::default(),
            complete_in_start: AtomicBool::new(false),
            hold_start: AtomicBool::new(false),
            start_release: Gate::default(),
            start_failure,
            hold_retirement,
        })
    }
    fn complete(&self) -> TestResult {
        let signal = lock(&self.signal).clone().ok_or("Fake process wait has not registered")?;
        signal.complete();
        Ok(())
    }
}
struct Input(Arc<Boundary>);
impl ProcessLease for Input {
    fn start(
        &mut self,
        correlation: NativeCorrelation,
        signal: Arc<Signal>,
    ) -> Result<(), NativeObservationError> {
        *lock(&self.0.signal) = Some(signal.clone());
        self.0.correlation.store(correlation.get(), Ordering::Release);
        self.0.starts.fetch_add(1, Ordering::AcqRel);
        self.0.started.open();
        if self.0.hold_start.load(Ordering::Acquire) {
            self.0.start_release.wait().map_err(|cause| {
                NativeObservationError::new(
                    "register_process_wait",
                    std::io::Error::other(cause.to_string()),
                )
            })?;
        }
        if self.0.complete_in_start.load(Ordering::Acquire) {
            signal.complete();
        }
        if self.0.panic_start.swap(false, Ordering::AcqRel) {
            std::panic::resume_unwind(Box::new("native start panic".to_owned()));
        }
        if self.0.start_failure {
            Err(NativeObservationError::new(
                "register_process_wait",
                std::io::Error::from_raw_os_error(5),
            ))
        } else {
            Ok(())
        }
    }
    fn read_exit(&mut self) -> Result<ProcessExit, NativeObservationError> {
        if self.0.panic_read.swap(false, Ordering::AcqRel) {
            std::panic::resume_unwind(Box::new("native read panic".to_owned()));
        }
        Ok(ProcessExit {
            correlation: NativeCorrelation::new(self.0.correlation.load(Ordering::Acquire))
                .map_err(|cause| NativeObservationError::new("fake_correlation", cause))?,
            process_id: 71,
            creation_time_100ns: 99,
            exit_code: 259,
        })
    }
    fn retire(&mut self) -> Result<(), NativeObservationError> {
        self.0.retiring.open();
        if self.0.panic_retire.swap(false, Ordering::AcqRel) {
            std::panic::resume_unwind(Box::new("native retirement panic".to_owned()));
        }
        if self.0.hold_retirement {
            self.0.release.wait().map_err(|cause| {
                NativeObservationError::new(
                    "quiesce_callback",
                    std::io::Error::other(cause.to_string()),
                )
            })?;
        }
        if self
            .0
            .retire_failures
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| count.checked_sub(1))
            .is_ok()
        {
            let code = self.0.retire_error.load(Ordering::Acquire);
            if code == 32 {
                self.0.second_error.open();
            }
            return Err(NativeObservationError::new(
                if code == 32 { "close_process_duplicate" } else { "unregister_process_wait" },
                std::io::Error::from_raw_os_error(code),
            ));
        }
        self.0.closes.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }
}
impl Drop for Input {
    fn drop(&mut self) {
        self.0.drops.fetch_add(1, Ordering::AcqRel);
    }
}
fn native_scope(rig: &Rig) -> TestResult<Scope> {
    Ok(rig.client.core.create_scope_kind(
        ScopeSpec { id: ScopeId::new(2)?, deadline: None },
        Some(&rig.scope),
        true,
    )?)
}
fn native_spec(id: u64) -> TestResult<JobSpec> {
    let mut spec = spec(id, JobClass::NativeObservation, 4096)?;
    spec.operation = Operation::NativeLifecycle;
    Ok(spec)
}
fn permit(rig: &Rig, scope: &Scope, id: u64) -> TestResult<ProcessWatchPermit> {
    Ok(reserve(&rig.client.core, scope, native_spec(id)?, NativeCorrelation::new(id)?)?)
}

#[test]
fn native_identity_class_scope_and_minimum_charge_reject_before_binding() -> TestResult {
    let rig = Rig::new()?;
    let scope = native_scope(&rig)?;
    assert!(NativeCorrelation::new(0).is_err());
    assert!(matches!(
        reserve(&rig.client.core, &rig.scope, native_spec(1)?, NativeCorrelation::new(1)?),
        Err(AdmissionError::NativeScopeRequired)
    ));
    let mut insufficient = native_spec(1)?;
    insufficient.reservation_bytes = 4095;
    assert!(matches!(
        reserve(&rig.client.core, &scope, insufficient, NativeCorrelation::new(1)?),
        Err(AdmissionError::NativeReservationTooSmall {
            requested_bytes: 4095,
            minimum_bytes: 4096
        })
    ));
    assert!(
        rig.client
            .submit_async(&scope, native_spec(1)?, |_| async { Ok::<_, String>(()) })
            .is_err()
    );
    assert!(rig.client.submit_blocking(&scope, native_spec(1)?, |_| Ok::<_, String>(())).is_err());
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    let reserved = permit(&rig, &scope, 1)?;
    assert_eq!(scope.snapshot().queued, 1);
    assert_eq!(scope.snapshot().reserved_bytes, 4096);
    drop(reserved);
    Ok(())
}

#[test]
fn bind_has_no_second_admission_and_observes_full_exit_identity() -> TestResult {
    let rig = Rig::new()?;
    let scope = native_scope(&rig)?;
    let reserved = permit(&rig, &scope, 1)?;
    let boundary = Boundary::new(false, false);
    let mut job = reserved.bind_input(Box::new(Input(boundary.clone())));
    boundary.started.wait()?;
    assert_eq!(job.status().phase, JobPhase::Running);
    boundary.complete()?;
    let completion = rig.completion(&mut job)?;
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    let JobOutcome::Success(exit) = completion.outcome else {
        return Err("Native exit was not successful".into());
    };
    assert_eq!(
        exit,
        ProcessExit {
            correlation: NativeCorrelation::new(1)?,
            process_id: 71,
            creation_time_100ns: 99,
            exit_code: 259
        }
    );
    assert_eq!(scope.snapshot().reserved_bytes, 4096);
    drop(completion.memory);
    drop(rig.owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn cancellation_retains_job_and_bytes_until_actual_retirement_returns() -> TestResult {
    let mut rig = Rig::new()?;
    let scope = native_scope(&rig)?;
    let boundary = Boundary::new(true, false);
    let mut job = permit(&rig, &scope, 1)?.bind_input(Box::new(Input(boundary.clone())));
    boundary.started.wait()?;
    job.cancel();
    boundary.retiring.wait()?;
    assert!(job.try_take_completion().is_none());
    assert_eq!(boundary.closes.load(Ordering::Acquire), 0);
    assert_eq!(scope.snapshot().reserved_bytes, 4096);
    rig.clock.advance(250);
    rig.owner.request_shutdown(Moment::from_nanos(0));
    let progress = rig.owner.poll_shutdown()?;
    assert!(!progress.settled);
    assert!(progress.deadline_expired);
    assert_eq!(progress.unresolved_job_ids, vec![JobId::new(1)?]);
    boundary.release.open();
    let completion = rig.completion(&mut job)?;
    assert!(matches!(completion.outcome, JobOutcome::Cancelled));
    assert_eq!(completion.execution_time, std::time::Duration::from_nanos(250));
    drop(completion);
    drop(rig.owner);
    assert_eq!(scope.snapshot().reserved_bytes, 0);
    Ok(())
}

#[test]
fn partial_registration_failure_cleans_before_original_typed_failure() -> TestResult {
    let rig = Rig::new()?;
    let scope = native_scope(&rig)?;
    let boundary = Boundary::new(true, true);
    let mut job = permit(&rig, &scope, 1)?.bind_input(Box::new(Input(boundary.clone())));
    boundary.retiring.wait()?;
    assert!(job.try_take_completion().is_none());
    boundary.release.open();
    let completion = rig.completion(&mut job)?;
    let JobOutcome::Failure(error) = completion.outcome else {
        return Err("Missing original native registration failure".into());
    };
    assert_eq!(error.stage(), "register_process_wait");
    assert_eq!(
        error
            .source()
            .and_then(|cause| cause.downcast_ref::<std::io::Error>())
            .and_then(std::io::Error::raw_os_error),
        Some(5)
    );
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    Ok(())
}

#[test]
fn deadline_retires_active_wait_without_detaching_its_lifetime() -> TestResult {
    let rig = Rig::new()?;
    let scope = native_scope(&rig)?;
    let boundary = Boundary::new(false, false);
    let deadline = Moment::from_nanos(100);
    let mut spec = native_spec(1)?;
    spec.deadline = Some(deadline);
    let mut job = reserve(&rig.client.core, &scope, spec, NativeCorrelation::new(1)?)?
        .bind_input(Box::new(Input(boundary.clone())));
    boundary.started.wait()?;
    rig.clock.wait_until_registered(deadline)?;
    rig.clock.advance(100);
    assert!(matches!(rig.completion(&mut job)?.outcome, JobOutcome::TimedOut));
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    Ok(())
}

#[path = "native_capacity_tests.rs"]
mod capacity;

#[path = "native_lifecycle_tests.rs"]
mod lifecycle;

#[path = "native_failure_tests.rs"]
mod failure;

#[path = "native_mailbox_tests.rs"]
mod mailbox;
