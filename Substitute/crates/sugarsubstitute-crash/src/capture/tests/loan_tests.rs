// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Hold actual finite work while testing the retained loan, reentry and unwind.
use super::*;
use std::sync::atomic::AtomicBool;

struct Release(Arc<Gate>);
impl Drop for Release {
    fn drop(&mut self) {
        self.0.open();
    }
}

pub(super) fn start(rig: &Rig, port: &Arc<FakePort>) -> TestResult<WindowsCrashCapture> {
    let mut capture = WindowsCrashCapture::with_port(coordinator()?, port.clone());
    let budget = CaptureBudget::new(Moment::from_nanos(0))?;
    capture
        .submit_start(
            &rig.client,
            &rig.scope,
            work_spec(2, budget.until())?,
            budget,
            rig.watch(1)?,
            input()?,
        )
        .map_err(|_| "Actual start rejected")?;
    port.entered.wait()?;
    Ok(capture)
}

pub(super) fn terminal(
    rig: &Rig,
    capture: &mut WindowsCrashCapture,
) -> TestResult<JobOutcome<(), CrashError>> {
    let mut job = capture.start.take().ok_or("Original actual start missing")?;
    Ok(rig.completion(&mut job)?.outcome)
}

pub(super) fn lease_address(shared: &Shared) -> TestResult<usize> {
    let state = lock(&shared.native);
    assert!(!state.borrowed);
    let lease = state.lease.as_deref().ok_or("Original retained native lease missing")?;
    Ok(std::ptr::from_ref(lease) as *const () as usize)
}

fn checked_close(rig: &Rig, capture: &mut WindowsCrashCapture) -> TestResult {
    let cleanup =
        rig.client.create_scope(ScopeSpec { id: ScopeId::new(3)?, deadline: None }, None)?;
    capture.submit_close(
        &rig.client,
        &cleanup,
        close_spec(3, Moment::from_nanos(60_000_000_000))?,
    )?;
    rig.observe_until(capture, |capture| capture.snapshot().state == CaptureState::Closed)
}

pub(super) fn arm(
    rig: &Rig,
    shared: Arc<Shared>,
    probe: &Mutex<Option<LeaseProbe>>,
    action: ClockCallback,
) -> (Arc<AtomicBool>, Arc<Gate>) {
    let clock = rig.clock.clone();
    let unlocked = Arc::new(AtomicBool::new(false));
    let observed = Arc::new(Gate::default());
    let saved = unlocked.clone();
    let arrival = observed.clone();
    *lock(probe) = Some(Box::new(move |context| {
        clock.on_next_observation(Box::new(move || {
            // The only worker able to hold this lifetime lock is causally in
            // this callback. Timer/completion paths do not touch this Shared.
            let available = shared.native.try_lock().is_ok();
            saved.store(available, Ordering::Release);
            arrival.open();
            if available {
                action();
            }
        }));
        context.now();
    }));
    (unlocked, observed)
}

#[test]
fn blocked_start_keeps_actual_loan_and_scope_controls_available() -> TestResult {
    let rig = Rig::new()?;
    let port = Arc::new(FakePort::default());
    let _release = Release(port.release.clone());
    let mut capture = start(&rig, &port)?;
    let shared = capture.shared.as_ref().ok_or("Missing original Shared")?.clone();
    {
        let state = shared.native.try_lock().map_err(|_| "Native work holds lifetime mutex")?;
        assert!(state.borrowed);
        assert!(state.lease.is_none());
        assert!(!state.retired);
        assert_eq!(state.deadline, Some(Moment::from_nanos(60_000_000_000)));
    }
    assert!(
        super::super::loan::NativeLoan::take(
            shared.clone(),
            sugarsubstitute_contracts::crash::CaptureOperation::Retirement,
        )
        .is_err()
    );
    rig.scope.cancel();
    assert!(rig.scope.snapshot().cancellation_requested);
    assert!(rig.scope.snapshot().reserved_bytes >= crate::input::STORAGE_BYTES as u64);
    assert_eq!(capture.snapshot().state, CaptureState::Starting);
    assert_eq!(port.dropped.load(Ordering::Acquire), 0);
    port.release.open();
    assert!(matches!(terminal(&rig, &mut capture)?, JobOutcome::Cancelled));
    let original = port.start_address.load(Ordering::Acquire);
    assert_eq!(lease_address(&shared)?, original);
    checked_close(&rig, &mut capture)?;
    assert_eq!(port.retire_address.load(Ordering::Acquire), original);
    drop(shared);
    assert_eq!(port.dropped.load(Ordering::Acquire), 1);
    Ok(())
}

#[test]
fn supplied_start_clock_reentrantly_cancels_and_retains_partial_process() -> TestResult {
    let rig = Rig::new()?;
    let port = Arc::new(FakePort::default());
    let _release = Release(port.release.clone());
    port.healthy.store(1, Ordering::Release);
    port.partial_process.store(1, Ordering::Release);
    let mut capture = start(&rig, &port)?;
    let shared = capture.shared.as_ref().ok_or("Missing original Shared")?.clone();
    let scope = rig.scope.clone();
    let (unlocked, observed) = arm(
        &rig,
        shared.clone(),
        &port.start_probe,
        Box::new(move || {
            scope.cancel();
            assert!(scope.snapshot().cancellation_requested);
        }),
    );
    port.release.open();
    observed.wait()?;
    assert!(matches!(terminal(&rig, &mut capture)?, JobOutcome::Cancelled));
    assert!(unlocked.load(Ordering::Acquire));
    assert_eq!(port.watch_binds.load(Ordering::Acquire), 1);
    assert!(!lock(&shared.native).registered);
    assert_eq!(lease_address(&shared)?, port.start_address.load(Ordering::Acquire));
    checked_close(&rig, &mut capture)?;
    Ok(())
}

#[test]
fn supplied_start_clock_panic_restores_exact_lease_and_fatal_cause() -> TestResult {
    let rig = Rig::new()?;
    let port = Arc::new(FakePort::default());
    let _release = Release(port.release.clone());
    port.partial_process.store(1, Ordering::Release);
    let mut capture = start(&rig, &port)?;
    let shared = capture.shared.as_ref().ok_or("Missing original Shared")?.clone();
    let (unlocked, observed) = arm(
        &rig,
        shared.clone(),
        &port.start_probe,
        Box::new(|| {
            std::panic::resume_unwind(Box::new("original-parent-start-provider".to_owned()));
        }),
    );
    port.release.open();
    observed.wait()?;
    let mut job = capture.start.take().ok_or("Original actual start missing")?;
    let JobOutcome::Panicked(cause) =
        rig.fatal_completion(&mut job, "original-parent-start-provider")?.outcome
    else {
        return Err("Original provider panic missing".into());
    };
    assert_eq!(cause.as_str(), "original-parent-start-provider");
    assert!(unlocked.load(Ordering::Acquire));
    assert_eq!(lease_address(&shared)?, port.start_address.load(Ordering::Acquire));
    assert_eq!(port.dropped.load(Ordering::Acquire), 0);
    assert!(rig.scope.snapshot().reserved_bytes >= crate::input::STORAGE_BYTES as u64);
    assert!(rig.client.now().is_err_and(|cause| {
        std::error::Error::source(&cause)
            .is_some_and(|cause| cause.to_string().contains("original-parent-start-provider"))
    }));
    assert!(!lock(&shared.native).registered);
    assert!(!lock(&shared.native).retired);
    Ok(())
}

#[test]
fn native_start_panic_restores_identical_lease_before_later_checked_close() -> TestResult {
    let rig = Rig::new()?;
    let port = Arc::new(FakePort::default());
    let _release = Release(port.release.clone());
    port.partial_process.store(1, Ordering::Release);
    let mut capture = start(&rig, &port)?;
    let shared = capture.shared.as_ref().ok_or("Missing original Shared")?.clone();
    *lock(&port.start_probe) = Some(Box::new(|_| {
        std::panic::resume_unwind(Box::new("original-native-start-panic".to_owned()));
    }));
    port.release.open();
    let JobOutcome::Panicked(cause) = terminal(&rig, &mut capture)? else {
        return Err("Original native panic missing".into());
    };
    assert_eq!(cause.as_str(), "original-native-start-panic");
    let original = port.start_address.load(Ordering::Acquire);
    assert_eq!(lease_address(&shared)?, original);
    assert_eq!(port.dropped.load(Ordering::Acquire), 0);
    checked_close(&rig, &mut capture)?;
    assert_eq!(port.retire_address.load(Ordering::Acquire), original);
    drop(shared);
    assert_eq!(port.dropped.load(Ordering::Acquire), 1);
    Ok(())
}

#[test]
fn blocked_close_keeps_loan_original_budget_and_cancellation_available() -> TestResult {
    let rig = Rig::new()?;
    let port = Arc::new(FakePort::default());
    port.release.open();
    let mut capture = start(&rig, &port)?;
    assert!(matches!(terminal(&rig, &mut capture)?, JobOutcome::Failure(_)));
    let shared = capture.shared.as_ref().ok_or("Missing original Shared")?.clone();
    let original = lease_address(&shared)?;
    let entered = Arc::new(Gate::default());
    let release = Arc::new(Gate::default());
    let _release = Release(release.clone());
    let arrival = entered.clone();
    let finish = release.clone();
    *lock(&port.retire_probe) = Some(Box::new(move |_| {
        arrival.open();
        if let Err(cause) = finish.wait() {
            std::panic::resume_unwind(Box::new(cause.to_string()));
        }
    }));
    let deadline = Moment::from_nanos(10_000_000_000);
    capture.submit_close(&rig.client, &rig.scope, close_spec(3, deadline)?)?;
    entered.wait()?;
    {
        let state = shared.native.try_lock().map_err(|_| "Native close holds lifetime mutex")?;
        assert!(state.borrowed);
        assert_eq!(state.close_deadline, Some(deadline));
        assert!(!state.retired);
    }
    rig.scope.cancel();
    assert!(rig.scope.snapshot().cancellation_requested);
    assert_eq!(port.retire_address.load(Ordering::Acquire), original);
    assert_eq!(port.dropped.load(Ordering::Acquire), 0);
    release.open();
    rig.observe_until(&mut capture, |capture| capture.snapshot().state == CaptureState::Closed)?;
    assert_eq!(capture.close_deadline, Some(deadline));
    Ok(())
}

#[test]
fn supplied_close_clock_panic_preserves_original_deadline_and_retained_owner() -> TestResult {
    let rig = Rig::new()?;
    let port = Arc::new(FakePort::default());
    port.release.open();
    let mut capture = start(&rig, &port)?;
    assert!(matches!(terminal(&rig, &mut capture)?, JobOutcome::Failure(_)));
    let shared = capture.shared.as_ref().ok_or("Missing original Shared")?.clone();
    let original = lease_address(&shared)?;
    let (unlocked, observed) = arm(
        &rig,
        shared.clone(),
        &port.retire_probe,
        Box::new(|| {
            std::panic::resume_unwind(Box::new("original-parent-close-provider".to_owned()));
        }),
    );
    let deadline = Moment::from_nanos(10_000_000_000);
    capture.submit_close(&rig.client, &rig.scope, close_spec(3, deadline)?)?;
    observed.wait()?;
    let mut close = capture.close.take().ok_or("Original actual close missing")?;
    let JobOutcome::Panicked(cause) =
        rig.fatal_completion(&mut close, "original-parent-close-provider")?.outcome
    else {
        return Err("Original close provider panic missing".into());
    };
    assert_eq!(cause.as_str(), "original-parent-close-provider");
    assert!(unlocked.load(Ordering::Acquire));
    assert_eq!(lease_address(&shared)?, original);
    assert_eq!(lock(&shared.native).close_deadline, Some(deadline));
    assert!(!lock(&shared.native).retired);
    assert_eq!(port.dropped.load(Ordering::Acquire), 0);
    assert!(rig.scope.snapshot().reserved_bytes >= crate::input::STORAGE_BYTES as u64);
    assert!(rig.client.now().is_err());
    Ok(())
}
