// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Order real clock observations through execution-owned CPU jobs and controlled providers.

use super::*;
use crate::runtime_test_support::{Gate, Rig, TestResult, spec};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use sugarsubstitute_contracts::execution::{
    AdmissionError, CpuStep, CpuTask, ExecutionPort, JobClass, JobContext, JobOutcome, ScopeId,
    ScopeSpec,
};

struct OverlappingProvider {
    calls: AtomicUsize,
    initial: u64,
    held: u64,
    entered: Gate,
    release: Gate,
}
impl Clock for OverlappingProvider {
    fn now(&self) -> Moment {
        let value = match self.calls.fetch_add(1, Ordering::AcqRel) {
            0 => self.initial,
            1 => {
                self.entered.open();
                if let Err(cause) = self.release.wait() {
                    std::panic::resume_unwind(Box::new(cause.to_string()));
                }
                self.held
            }
            _ => 20,
        };
        Moment::from_nanos(value)
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}
struct Observe(Arc<RetirementClock>);
impl CpuTask<Moment, String> for Observe {
    fn step(&mut self, _: &JobContext) -> Result<CpuStep<Moment>, String> {
        self.0.metadata().map(CpuStep::Complete).map_err(|cause| cause.to_string())
    }
}

fn overlap(initial: u64, held: u64, rejected: bool) -> TestResult {
    let rig = Rig::with_workers(2)?;
    let provider = Arc::new(OverlappingProvider {
        calls: AtomicUsize::new(0),
        initial,
        held,
        entered: Gate::default(),
        release: Gate::default(),
    });
    let clock = Arc::new(RetirementClock::new(provider.clone(), Weak::new()));
    let first_clock = clock.clone();
    let mut first =
        rig.client.submit_cpu(&rig.scope, spec(1, JobClass::CpuInteractive, 0)?, move |_| {
            Observe(first_clock)
        })?;
    provider.entered.wait()?;
    let second_clock = clock.clone();
    let mut second =
        rig.client.submit_cpu(&rig.scope, spec(2, JobClass::CpuDecode, 0)?, move |_| {
            Observe(second_clock)
        })?;
    let second_result = rig.completion(&mut second);
    // Release before propagating an unexpected result so the owner always joins.
    provider.release.open();
    assert!(
        matches!(second_result?.outcome, JobOutcome::Success(at) if at == Moment::from_nanos(20))
    );
    let first_result = rig.completion(&mut first)?;
    if rejected {
        assert!(matches!(first_result.outcome, JobOutcome::Failure(_)));
        assert!(clock.failed());
        let error = clock.metadata().err().ok_or("Regression metadata was admitted")?;
        assert_eq!(
            std::error::Error::source(&error)
                .and_then(std::error::Error::source)
                .and_then(|cause| cause.downcast_ref::<TimeError>()),
            Some(&TimeError::Regression),
        );
        assert_eq!(clock.retirement_now(), Moment::from_nanos(20));
    } else {
        assert!(
            matches!(first_result.outcome, JobOutcome::Success(at) if at == Moment::from_nanos(20))
        );
        assert!(!clock.failed());
        assert_eq!(clock.metadata()?, Moment::from_nanos(20));
    }
    Ok(())
}

#[test]
fn legitimate_overlapping_samples_finish_out_of_order_without_regression() -> TestResult {
    overlap(0, 10, false)
}

#[test]
fn overlapping_completion_cannot_hide_regression_below_own_start_bound() -> TestResult {
    overlap(10, 5, true)
}

struct CallerRegression {
    caller: std::thread::ThreadId,
    regress: AtomicBool,
}
impl Clock for CallerRegression {
    fn now(&self) -> Moment {
        Moment::from_nanos(
            if std::thread::current().id() == self.caller && self.regress.load(Ordering::Acquire) {
                5
            } else {
                10
            },
        )
    }
    fn sleep_until(&self, _: Moment) -> WaitFuture {
        Box::pin(std::future::pending())
    }
}

#[test]
fn unlatched_regression_fences_admission_before_lock_free_owner_shutdown() -> TestResult {
    use crate::runtime_test_support::Wake;
    let provider = Arc::new(CallerRegression {
        caller: std::thread::current().id(),
        regress: AtomicBool::new(false),
    });
    let owner = crate::ExecutionOwner::new(provider.clone(), Arc::new(Wake::default()))?;
    let client = owner.client();
    let state = lock(&client.core.state);
    provider.regress.store(true, Ordering::Release);
    let payload = client.core.clock.observe_unlatched().err().ok_or("Regression not fenced")?;
    assert_eq!(payload.downcast_ref::<TimeError>(), Some(&TimeError::Regression));
    assert!(client.core.clock.failed());
    assert!(!state.stopping);
    drop(state);
    assert!(matches!(
        client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None),
        Err(AdmissionError::ShuttingDown)
    ));
    assert!(
        catch_unwind(AssertUnwindSafe(|| client.core.clock.fail_observation(payload))).is_err()
    );
    assert!(lock(&client.core.state).stopping);
    assert_eq!(client.core.clock.retirement_now(), Moment::from_nanos(10));
    Ok(())
}
