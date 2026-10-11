// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Qualify period ownership, inactive modes and native failure recovery without timer sleeps.

use super::*;
use std::cell::RefCell;

type TestResult = Result<(), Box<dyn Error>>;

#[derive(Default)]
struct State {
    calls: Vec<(&'static str, u32)>,
    begin_result: u32,
    end_result: u32,
}
struct FakePort(Rc<RefCell<State>>);
impl PeriodPort for FakePort {
    fn begin(&mut self, milliseconds: u32) -> Result<(), DesktopError> {
        let mut state = self.0.borrow_mut();
        state.calls.push(("begin", milliseconds));
        native_result("begin_animation_period", milliseconds, state.begin_result)
    }
    fn end(&mut self, milliseconds: u32) -> Result<(), DesktopError> {
        let mut state = self.0.borrow_mut();
        state.calls.push(("end", milliseconds));
        native_result("end_animation_period", milliseconds, state.end_result)
    }
}
fn fixture() -> (AnimationPeriod<FakePort>, Rc<RefCell<State>>) {
    let state = Rc::new(RefCell::new(State::default()));
    (AnimationPeriod::with_port(FakePort(state.clone())), state)
}

#[test]
fn unchanged_animation_and_inactive_modes_make_no_extra_native_calls() -> TestResult {
    let (mut period, state) = fixture();
    period.reconcile(false)?;
    assert!(state.borrow().calls.is_empty());
    for _ in 0..100 {
        period.reconcile(true)?;
    }
    assert_eq!(state.borrow().calls, [("begin", 1)]);
    for _ in 0..100 {
        period.reconcile(false)?;
    }
    period.shutdown()?;
    period.reconcile(true)?;
    assert!(!period.active);
    drop(period);
    assert_eq!(state.borrow().calls, [("begin", 1), ("end", 1)]);
    Ok(())
}

#[test]
fn static_hidden_minimized_and_closing_release_each_visible_animation_acquisition() -> TestResult {
    let (mut period, state) = fixture();
    for (visible, animating, closing) in
        [(true, false, false), (false, true, false), (false, true, false), (true, true, true)]
    {
        period.reconcile(true)?;
        period.reconcile(visible && animating && !closing)?;
        assert!(!period.active);
    }
    drop(period);
    assert_eq!(state.borrow().calls, [("begin", 1), ("end", 1)].repeat(4));
    Ok(())
}

#[test]
fn begin_failure_preserves_original_winmm_code_and_never_releases_an_unowned_request() -> TestResult
{
    let (mut period, state) = fixture();
    state.borrow_mut().begin_result = 97;
    let error = period.reconcile(true).err().ok_or("Begin failure missing")?;
    assert_eq!(error.to_string(), "begin_animation_period: WinMM 1ms period returned code 97");
    assert_eq!(
        error
            .source()
            .and_then(|cause| cause.downcast_ref::<WinMmError>())
            .ok_or("Typed result missing")?
            .result,
        97
    );
    assert!(!period.active);
    period.reconcile(true)?;
    period.reconcile(false)?;
    period.shutdown()?;
    drop(period);
    assert_eq!(state.borrow().calls, [("begin", 1)]);
    Ok(())
}

#[test]
fn failed_end_retains_owned_period_and_freezes_callback_retries_until_explicit_shutdown()
-> TestResult {
    let (mut period, state) = fixture();
    period.reconcile(true)?;
    state.borrow_mut().end_result = 97;
    let error = period.reconcile(false).err().ok_or("End failure missing")?;
    assert_eq!(error.to_string(), "end_animation_period: WinMM 1ms period returned code 97");
    assert!(period.active);
    for _ in 0..100 {
        period.reconcile(false)?;
    }
    assert_eq!(state.borrow().calls, [("begin", 1), ("end", 1)]);
    assert!(period.shutdown().is_err());
    assert!(period.active);
    state.borrow_mut().end_result = 0;
    period.shutdown()?;
    assert!(!period.active);
    drop(period);
    assert_eq!(state.borrow().calls, [("begin", 1), ("end", 1), ("end", 1), ("end", 1)]);
    Ok(())
}

#[test]
fn drop_balances_early_return_and_final_release_failure_remains_observable() -> TestResult {
    let (mut period, state) = fixture();
    period.reconcile(true)?;
    drop(period);
    assert_eq!(state.borrow().calls, [("begin", 1), ("end", 1)]);
    let (mut period, state) = fixture();
    period.reconcile(true)?;
    state.borrow_mut().end_result = 97;
    assert!(period.shutdown().is_err());
    drop(period);
    assert_eq!(state.borrow().calls, [("begin", 1), ("end", 1), ("end", 1)]);
    Ok(())
}

#[test]
fn real_winmm_period_acquires_and_releases_without_waiting_or_creating_windows() -> TestResult {
    let mut period = WindowsAnimationPeriod::new();
    period.reconcile(true)?;
    assert!(period.active);
    period.shutdown()?;
    assert!(!period.active);
    Ok(())
}
