// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Supervise native clock failure and retain unresolved cleanup without spinning.

use super::*;
use crate::runtime_test_support::{ClockPort, ManualClock, Wake};

struct NativePanicClock {
    clock: Arc<ManualClock>,
    armed: Arc<AtomicBool>,
}
impl Clock for NativePanicClock {
    fn now(&self) -> Moment {
        if std::thread::current().name() == Some("sugarsubstitute-native-retirement")
            && self.armed.swap(false, Ordering::AcqRel)
        {
            std::panic::resume_unwind(Box::new("native clock panic".to_owned()));
        }
        ClockPort(self.clock.clone()).now()
    }
    fn sleep_until(&self, deadline: Moment) -> WaitFuture {
        ClockPort(self.clock.clone()).sleep_until(deadline)
    }
}

#[test]
fn registered_wait_survives_native_clock_panic_until_actual_retirement() -> TestResult {
    let clock = Arc::new(ManualClock::default());
    let armed = Arc::new(AtomicBool::new(false));
    let wake = Arc::new(Wake::default());
    let owner = crate::ExecutionOwner::with_cpu_workers(
        Arc::new(NativePanicClock { clock: clock.clone(), armed: armed.clone() }),
        wake.clone(),
        2,
    )?;
    let client = owner.client();
    let ordinary = client.create_scope(ScopeSpec { id: ScopeId::new(1)?, deadline: None }, None)?;
    let rig = Rig { owner, client, clock, wake, scope: ordinary };
    let scope = native_scope(&rig)?;
    let boundary = Boundary::new(true, false);
    let mut job = permit(&rig, &scope, 1)?.bind_input(Box::new(Input(boundary.clone())));
    boundary.started.wait()?;
    armed.store(true, Ordering::Release);
    rig.client.core.changed();
    boundary.retiring.wait()?;
    assert!(lock(&rig.client.core.state).stopping);
    assert!(job.try_take_completion().is_none());
    assert_eq!(scope.snapshot().reserved_bytes, 4096);
    assert_eq!(boundary.drops.load(Ordering::Acquire), 0);
    boundary.release.open();
    let (completion, _) = super::lifecycle::terminal_with_failures(&rig, &mut job)?;
    let JobOutcome::Panicked(cause) = completion.outcome else {
        return Err("Native clock panic was not retained".into());
    };
    assert_eq!(cause.as_str(), "native clock panic");
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    Ok(())
}

#[test]
fn unregister_failure_retains_registration_credit_and_first_cause_until_retry_settles() -> TestResult
{
    let rig = Rig::new()?;
    let scope = native_scope(&rig)?;
    let boundary = Boundary::new(false, false);
    boundary.retire_failures.store(usize::MAX, Ordering::Release);
    let mut job = permit(&rig, &scope, 1)?.bind_input(Box::new(Input(boundary.clone())));
    boundary.started.wait()?;
    let observed = rig.wake.count();
    job.cancel();
    rig.wake.wait_after(observed)?;
    assert!(job.try_take_completion().is_none());
    assert_eq!(boundary.closes.load(Ordering::Acquire), 0);
    assert_eq!(boundary.drops.load(Ordering::Acquire), 0);
    assert_eq!(scope.snapshot().reserved_bytes, 4096);
    boundary.retire_failures.store(0, Ordering::Release);
    rig.client.core.changed();
    let (completion, _) = super::lifecycle::terminal_with_failures(&rig, &mut job)?;
    let JobOutcome::Failure(cause) = completion.outcome else {
        return Err("Native unregister failure was not retained".into());
    };
    assert_eq!(cause.stage(), "unregister_process_wait");
    assert_eq!(
        cause
            .source()
            .and_then(|error| error.downcast_ref::<std::io::Error>())
            .and_then(std::io::Error::raw_os_error),
        Some(6)
    );
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    Ok(())
}

#[test]
fn distinct_cleanup_errors_remain_owned_after_original_registration_failure() -> TestResult {
    let rig = Rig::new()?;
    let scope = native_scope(&rig)?;
    let boundary = Boundary::new(true, true);
    boundary.retire_failures.store(usize::MAX, Ordering::Release);
    let mut job = permit(&rig, &scope, 1)?.bind_input(Box::new(Input(boundary.clone())));
    boundary.retiring.wait()?;
    let observed = rig.wake.count();
    boundary.release.open();
    rig.wake.wait_after(observed)?;
    boundary.retire_error.store(32, Ordering::Release);
    rig.client.core.changed();
    boundary.second_error.wait()?;
    boundary.retire_failures.store(0, Ordering::Release);
    rig.client.core.changed();
    let (completion, _) = super::lifecycle::terminal_with_failures(&rig, &mut job)?;
    let JobOutcome::Failure(error) = completion.outcome else {
        return Err("Original failure disappeared".into());
    };
    assert_eq!(error.stage(), "register_process_wait");
    let code = |error: &NativeObservationError| {
        error
            .source()
            .and_then(|error| error.downcast_ref::<std::io::Error>())
            .and_then(std::io::Error::raw_os_error)
    };
    assert_eq!(code(&error), Some(5));
    assert!(error.cleanup.iter().any(|error| code(error) == Some(6)));
    assert!(error.cleanup.iter().any(|error| code(error) == Some(32)));
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    Ok(())
}

#[test]
fn cleanup_cause_inventory_is_bounded_and_reports_omission_without_replacing_source() {
    let mut error =
        NativeObservationError::new("register_process_wait", std::io::Error::from_raw_os_error(5));
    for code in 6..=15 {
        error.retain_cleanup(NativeObservationError::new(
            "close_process_duplicate",
            std::io::Error::from_raw_os_error(code),
        ));
    }
    assert_eq!(error.cleanup.len(), 8);
    assert_eq!(error.omitted_cleanup, 2);
    assert!(error.to_string().contains("2 further cleanup failures omitted"));
    assert_eq!(
        error
            .source()
            .and_then(|error| error.downcast_ref::<std::io::Error>())
            .and_then(std::io::Error::raw_os_error),
        Some(5)
    );
}

#[test]
fn callback_during_registration_observes_durable_context_and_full_exit_code() -> TestResult {
    let rig = Rig::new()?;
    let scope = native_scope(&rig)?;
    let boundary = Boundary::new(false, false);
    boundary.complete_in_start.store(true, Ordering::Release);
    let mut job = permit(&rig, &scope, 1)?.bind_input(Box::new(Input(boundary.clone())));
    let completion = rig.completion(&mut job)?;
    let JobOutcome::Success(exit) = completion.outcome else {
        return Err("Early callback did not yield typed exit".into());
    };
    assert_eq!(exit.exit_code, 259);
    assert_eq!(boundary.starts.load(Ordering::Acquire), 1);
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    Ok(())
}

#[test]
fn partial_registration_failure_with_delivered_callback_retains_owner_until_cleanup() -> TestResult
{
    let rig = Rig::new()?;
    let scope = native_scope(&rig)?;
    let boundary = Boundary::new(true, true);
    boundary.complete_in_start.store(true, Ordering::Release);
    let mut job = permit(&rig, &scope, 1)?.bind_input(Box::new(Input(boundary.clone())));
    boundary.retiring.wait()?;
    assert!(job.try_take_completion().is_none());
    assert_eq!(boundary.drops.load(Ordering::Acquire), 0);
    boundary.release.open();
    let completion = rig.completion(&mut job)?;
    let JobOutcome::Failure(error) = completion.outcome else {
        return Err("Partial registration original cause lost".into());
    };
    assert_eq!(error.stage(), "register_process_wait");
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    Ok(())
}

#[test]
fn deadline_expiring_during_registration_retires_before_accepting_signalled_success() -> TestResult
{
    let rig = Rig::new()?;
    let scope = native_scope(&rig)?;
    let boundary = Boundary::new(false, false);
    boundary.hold_start.store(true, Ordering::Release);
    boundary.complete_in_start.store(true, Ordering::Release);
    let mut request = native_spec(1)?;
    let deadline = Moment::from_nanos(100);
    request.deadline = Some(deadline);
    let mut job = reserve(&rig.client.core, &scope, request, NativeCorrelation::new(1)?)?
        .bind_input(Box::new(Input(boundary.clone())));
    boundary.started.wait()?;
    rig.clock.wait_until_registered(deadline)?;
    rig.clock.advance(100);
    assert!(job.try_take_completion().is_none());
    boundary.start_release.open();
    let completion = rig.completion(&mut job)?;
    assert!(matches!(completion.outcome, JobOutcome::TimedOut));
    assert_eq!(completion.execution_time, std::time::Duration::from_nanos(100));
    assert_eq!(boundary.closes.load(Ordering::Acquire), 1);
    Ok(())
}
