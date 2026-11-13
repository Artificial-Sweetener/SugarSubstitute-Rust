// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Drive the production policy helper with owned boundary facts, without a native loop.
use super::*;
use std::{
    cell::RefCell,
    collections::VecDeque,
    error::Error,
    fmt,
    rc::Rc,
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};
use sugarsubstitute_contracts::execution::ExecutionError;
use sugarsubstitute_execution::qualification_time::anchor;
type TestResult = Result<(), Box<dyn Error>>;
#[derive(Debug)]
struct Cause(&'static str);
impl fmt::Display for Cause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}
impl Error for Cause {}
struct CountWake(Arc<AtomicUsize>);
impl CompletionWake for CountWake {
    fn wake(&self) -> Result<(), ExecutionError> {
        self.0.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}
#[derive(Default)]
struct Lifetime {
    steps: VecDeque<HeadlessStep>,
    preparations: VecDeque<Result<super::super::StartupPreparationStep, Box<dyn Error>>>,
    preparation_budgets: Vec<usize>,
    calls: Rc<RefCell<Vec<&'static str>>>,
    budgets: Vec<usize>,
    deadline: Option<Instant>,
    start_error: bool,
    activity_error: bool,
    request_error: bool,
    poll_error: bool,
    finish_error: bool,
    settled: bool,
    requests: usize,
    finishes: usize,
}
impl HeadlessLifetime for Lifetime {
    fn start(&mut self, wake: Arc<dyn CompletionWake>) -> Result<(), Box<dyn Error>> {
        self.calls.borrow_mut().push("start");
        wake.wake()?;
        if self.start_error {
            return Err(Box::new(Cause("start")));
        }
        Ok(())
    }
    fn prepare_native_runtime(
        &mut self,
        max: NonZeroUsize,
    ) -> Result<super::super::StartupPreparationStep, Box<dyn Error>> {
        self.calls.borrow_mut().push("prepare");
        self.preparation_budgets.push(max.get());
        self.preparations.pop_front().unwrap_or(Ok(super::super::StartupPreparationStep::Waiting))
    }
    fn activity(&mut self, max: NonZeroUsize) -> Result<HeadlessStep, Box<dyn Error>> {
        self.calls.borrow_mut().push("activity");
        self.budgets.push(max.get());
        if self.activity_error {
            self.activity_error = false;
            return Err(Box::new(Cause("activity")));
        }
        Ok(self.steps.pop_front().unwrap_or(HeadlessStep::Active))
    }
    fn next_deadline(&self) -> Option<Instant> {
        self.deadline
    }
    fn request_shutdown(&mut self) -> Result<(), Box<dyn Error>> {
        self.calls.borrow_mut().push("request");
        self.requests += 1;
        if self.request_error {
            return Err(Box::new(Cause("request")));
        }
        Ok(())
    }
    fn poll_shutdown(&mut self) -> Result<bool, Box<dyn Error>> {
        self.calls.borrow_mut().push("poll");
        if self.poll_error {
            self.poll_error = false;
            return Err(Box::new(Cause("poll")));
        }
        Ok(self.settled)
    }
    fn finish_shutdown(&mut self) -> Result<(), Box<dyn Error>> {
        self.calls.borrow_mut().push("finish");
        self.finishes += 1;
        if self.finish_error {
            return Err(Box::new(Cause("finish")));
        }
        Ok(())
    }
}
fn start(state: &mut HeadlessState, lifetime: &mut Lifetime) -> Arc<AtomicUsize> {
    let count = Arc::new(AtomicUsize::new(0));
    state.start(lifetime, Arc::new(CountWake(count.clone())));
    count
}
fn cause(error: &DesktopError) -> Option<&'static str> {
    error.source().and_then(|cause| cause.downcast_ref::<Cause>()).map(|cause| cause.0)
}

#[test]
fn active_wait_uses_only_owned_earliest_deadline_without_periodic_polling() {
    let now = anchor();
    let mut lifetime = Lifetime::default();
    let state = HeadlessState::new();
    assert_eq!(state.control(&lifetime), ControlFlow::Wait);
    for deadline in [now, now + Duration::from_secs(5), now + Duration::from_secs(30)] {
        lifetime.deadline = Some(deadline);
        assert_eq!(state.control(&lifetime), ControlFlow::WaitUntil(deadline));
    }
    assert!(lifetime.calls.borrow().is_empty());
}
#[test]
fn activity_has_fixed_budget_and_start_accepts_early_boundary_wake() {
    let mut lifetime = Lifetime::default();
    let mut state = HeadlessState::new();
    let count = start(&mut state, &mut lifetime);
    assert_eq!(count.load(Ordering::Relaxed), 1);
    for _ in 0..3 {
        state.activity(&mut lifetime);
    }
    assert_eq!(lifetime.budgets, [32, 32, 32]);
    state.poll_shutdown(&mut lifetime);
    assert_eq!(lifetime.requests, 0);
    assert_eq!(lifetime.calls.borrow().iter().filter(|call| **call == "start").count(), 1);
    assert!(!lifetime.calls.borrow().contains(&"poll"));
}
#[test]
fn shutdown_requests_once_and_keeps_draining_until_actual_settlement() -> TestResult {
    let now = anchor();
    let mut lifetime = Lifetime {
        steps: VecDeque::from([HeadlessStep::Shutdown, HeadlessStep::Shutdown]),
        ..Lifetime::default()
    };
    let mut state = HeadlessState::new();
    start(&mut state, &mut lifetime);
    state.activity(&mut lifetime);
    state.activity(&mut lifetime);
    assert_eq!(lifetime.requests, 1);
    // Closing waits only for actual completion wakes and the owner's own deadlines.
    assert_eq!(state.control(&lifetime), ControlFlow::Wait);
    for deadline in [now + Duration::from_millis(3), now + Duration::from_secs(10)] {
        lifetime.deadline = Some(deadline);
        assert_eq!(state.control(&lifetime), ControlFlow::WaitUntil(deadline));
    }
    state.poll_shutdown(&mut lifetime);
    assert!(!state.settled());
    assert_eq!(lifetime.finishes, 0);
    lifetime.settled = true;
    state.poll_shutdown(&mut lifetime);
    assert!(state.settled());
    assert_eq!(state.control(&lifetime), ControlFlow::Exit);
    let calls = lifetime.calls.clone();
    state.finish(&mut lifetime, || calls.borrow_mut().push("native_teardown"))?;
    assert_eq!(
        &lifetime.calls.borrow()[lifetime.calls.borrow().len() - 2..],
        ["native_teardown", "finish"]
    );
    assert_eq!(lifetime.finishes, 1);
    Ok(())
}
#[test]
fn first_start_failure_survives_shutdown_poll_and_finish_failures() -> TestResult {
    let mut lifetime = Lifetime {
        start_error: true,
        request_error: true,
        poll_error: true,
        finish_error: true,
        ..Lifetime::default()
    };
    let mut state = HeadlessState::new();
    start(&mut state, &mut lifetime);
    state.poll_shutdown(&mut lifetime);
    assert!(!state.settled());
    state.activity(&mut lifetime);
    assert_eq!(lifetime.requests, 1);
    lifetime.settled = true;
    state.poll_shutdown(&mut lifetime);
    let error = state.finish(&mut lifetime, || {}).err().ok_or("startup failure was lost")?;
    assert_eq!(cause(&error), Some("start"));
    assert_eq!(lifetime.finishes, 1);
    Ok(())
}
#[test]
fn activity_failure_enters_same_close_path_and_preserves_original_cause() -> TestResult {
    let mut lifetime =
        Lifetime { activity_error: true, request_error: true, ..Lifetime::default() };
    let mut state = HeadlessState::new();
    start(&mut state, &mut lifetime);
    state.activity(&mut lifetime);
    assert_eq!(lifetime.requests, 1);
    assert!(!state.settled());
    lifetime.settled = true;
    state.poll_shutdown(&mut lifetime);
    let error = state.finish(&mut lifetime, || {}).err().ok_or("activity failure was lost")?;
    assert_eq!(cause(&error), Some("activity"));
    Ok(())
}
#[test]
fn shutdown_request_failure_is_not_settlement_or_a_repeated_request() -> TestResult {
    let mut lifetime = Lifetime {
        request_error: true,
        steps: VecDeque::from([HeadlessStep::Shutdown, HeadlessStep::Shutdown]),
        ..Lifetime::default()
    };
    let mut state = HeadlessState::new();
    state.activity(&mut lifetime);
    state.activity(&mut lifetime);
    state.poll_shutdown(&mut lifetime);
    assert_eq!(lifetime.requests, 1);
    assert!(!state.settled());
    lifetime.settled = true;
    state.poll_shutdown(&mut lifetime);
    let error = state.finish(&mut lifetime, || {}).err().ok_or("shutdown failure was lost")?;
    assert_eq!(cause(&error), Some("request"));
    Ok(())
}
#[test]
fn early_native_return_preserves_owner_and_never_authorizes_finish() -> TestResult {
    let mut lifetime = Lifetime::default();
    let mut state = HeadlessState::new();
    state.native_return(&mut lifetime, 0);
    assert_eq!(lifetime.requests, 1);
    assert!(!state.settled());
    state.native_return(&mut lifetime, 0);
    assert_eq!(lifetime.requests, 1);
    let calls = lifetime.calls.clone();
    let error = state
        .finish(&mut lifetime, || calls.borrow_mut().push("native_teardown"))
        .err()
        .ok_or("unsettled exit accepted")?;
    assert!(error.to_string().starts_with("headless_native_loop:"));
    assert_eq!(lifetime.finishes, 0);
    assert!(!lifetime.calls.borrow().contains(&"native_teardown"));
    Ok(())
}
#[test]
fn early_native_return_requires_real_settlement_and_retains_native_status_error() -> TestResult {
    let mut lifetime = Lifetime::default();
    let mut state = HeadlessState::new();
    state.native_return(&mut lifetime, 17);
    state.poll_shutdown(&mut lifetime);
    assert!(!state.settled());
    lifetime.settled = true;
    state.poll_shutdown(&mut lifetime);
    let error = state.finish(&mut lifetime, || {}).err().ok_or("native failure was lost")?;
    assert!(error.to_string().contains("status 17"));
    assert_eq!(lifetime.finishes, 1);
    Ok(())
}
#[test]
fn finish_failure_survives_after_successful_settlement_and_native_teardown() -> TestResult {
    let mut lifetime = Lifetime {
        settled: true,
        finish_error: true,
        steps: VecDeque::from([HeadlessStep::Shutdown]),
        ..Lifetime::default()
    };
    let mut state = HeadlessState::new();
    state.activity(&mut lifetime);
    state.poll_shutdown(&mut lifetime);
    state.native_return(&mut lifetime, 0);
    let calls = lifetime.calls.clone();
    let error = state
        .finish(&mut lifetime, || calls.borrow_mut().push("native_teardown"))
        .err()
        .ok_or("finish failure was lost")?;
    assert_eq!(cause(&error), Some("finish"));
    assert_eq!(
        &lifetime.calls.borrow()[lifetime.calls.borrow().len() - 2..],
        ["native_teardown", "finish"]
    );
    Ok(())
}

#[path = "preparation_tests.rs"]
mod preparation_tests;
