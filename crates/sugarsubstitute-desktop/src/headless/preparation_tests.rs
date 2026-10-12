// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Characterize pre-Tao headless admission and retained cleanup.
use super::*;
#[test]
fn preparation_requires_actual_ready_after_one_start_and_never_restarts_content() -> TestResult {
    let mut lifetime = Lifetime {
        preparations: VecDeque::from([
            Ok(crate::StartupPreparationStep::Waiting),
            Ok(crate::StartupPreparationStep::Ready),
        ]),
        ..Lifetime::default()
    };
    let mut state = HeadlessState::new();
    start(&mut state, &mut lifetime);
    state.prepare(&mut lifetime);
    assert!(!state.prepared());
    state.prepare(&mut lifetime);
    assert!(state.prepared());
    state.prepare(&mut lifetime);
    assert_eq!(lifetime.preparation_budgets, [32, 32]);
    assert_eq!(lifetime.calls.borrow().iter().filter(|c| **c == "start").count(), 1);
    assert_eq!(lifetime.calls.borrow().iter().filter(|c| **c == "prepare").count(), 2);
    Ok(())
}
#[test]
fn preparation_failure_closes_once_and_keeps_bounded_draining_until_actual_settlement() -> TestResult
{
    let mut lifetime = Lifetime {
        preparations: VecDeque::from([Err(Box::new(Cause("preparation")) as Box<dyn Error>)]),
        ..Lifetime::default()
    };
    let mut state = HeadlessState::new();
    start(&mut state, &mut lifetime);
    state.prepare(&mut lifetime);
    assert!(!state.prepared());
    assert!(!state.settled());
    state.prepare(&mut lifetime);
    assert_eq!(lifetime.requests, 1);
    assert_eq!(lifetime.budgets, [32, 32]);
    lifetime.settled = true;
    state.prepare(&mut lifetime);
    assert!(state.settled());
    let error = state.finish(&mut lifetime, || {}).err().ok_or("Failure lost")?;
    assert_eq!(cause(&error), Some("preparation"));
    assert_eq!(lifetime.finishes, 1);
    Ok(())
}
#[test]
fn post_preparation_native_failure_reenters_same_owner_cleanup_instead_of_admitting_tao()
-> TestResult {
    let mut lifetime = Lifetime {
        preparations: VecDeque::from([Ok(crate::StartupPreparationStep::Ready)]),
        ..Lifetime::default()
    };
    let mut state = HeadlessState::new();
    start(&mut state, &mut lifetime);
    state.prepare(&mut lifetime);
    assert!(state.prepared());
    state.fail(&mut lifetime, DesktopError::new("native_handoff", Cause("handoff")));
    assert!(!state.prepared());
    state.prepare(&mut lifetime);
    assert!(!state.settled());
    lifetime.settled = true;
    state.prepare(&mut lifetime);
    assert!(state.settled());
    assert_eq!(lifetime.requests, 1);
    assert_eq!(lifetime.budgets, [32, 32]);
    let error = state.finish(&mut lifetime, || {}).err().ok_or("Failure lost")?;
    assert_eq!(cause(&error), Some("handoff"));
    Ok(())
}
#[test]
fn cancelled_preparation_cannot_admit_a_late_ready_and_second_start_is_rejected() -> TestResult {
    let mut lifetime = Lifetime {
        preparations: VecDeque::from([
            Ok(crate::StartupPreparationStep::Closing),
            Ok(crate::StartupPreparationStep::Ready),
        ]),
        ..Lifetime::default()
    };
    let mut state = HeadlessState::new();
    let wake = start(&mut state, &mut lifetime);
    state.prepare(&mut lifetime);
    state.prepare(&mut lifetime);
    assert!(!state.prepared());
    assert_eq!(lifetime.preparation_budgets, [32]);
    state.start(&mut lifetime, Arc::new(CountWake(wake)));
    assert_eq!(lifetime.calls.borrow().iter().filter(|c| **c == "start").count(), 1);
    lifetime.settled = true;
    state.prepare(&mut lifetime);
    assert!(state.settled());
    assert_eq!(lifetime.requests, 1);
    Ok(())
}
#[test]
fn actual_pre_tao_start_failure_finishes_without_constructing_a_tao_loop() -> TestResult {
    let mut lifetime = Lifetime { start_error: true, settled: true, ..Lifetime::default() };
    let error = crate::run_headless(&mut lifetime).err().ok_or("Start failure accepted")?;
    assert_eq!(cause(&error), Some("start"));
    assert_eq!(lifetime.finishes, 1);
    assert_eq!(lifetime.requests, 1);
    assert!(!lifetime.calls.borrow().contains(&"prepare"));
    Ok(())
}
