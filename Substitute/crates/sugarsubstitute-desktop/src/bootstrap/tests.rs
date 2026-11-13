// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Inject primitive native event/proxy outcomes while exercising the real retained wake.
use super::*;
use std::{
    error::Error,
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};
use sugarsubstitute_contracts::execution::Operation;
use sugarsubstitute_execution::qualification_time::anchor;
type TestResult = Result<(), Box<dyn Error>>;
#[derive(Default)]
struct Facts {
    calls: Vec<&'static str>,
    closed: bool,
    signalled: bool,
    fail_reset: bool,
    fail_wait: bool,
    fail_close: bool,
    waits: Vec<(Option<Instant>, bool)>,
}
#[derive(Default)]
struct Event(Mutex<Facts>);
fn failure(stage: &'static str) -> ExecutionError {
    ExecutionError::new(Operation::NativeLifecycle, Box::new(std::io::Error::other(stage)))
}
impl EventBoundary for Event {
    fn signal(&self) -> Result<(), ExecutionError> {
        let mut f = self.0.lock().map_err(|_| failure("Fake event lock poisoned"))?;
        if f.closed {
            return Err(failure("signal closed event"));
        }
        f.calls.push("signal");
        f.signalled = true;
        Ok(())
    }
    fn reset(&self) -> Result<(), ExecutionError> {
        let mut f = self.0.lock().map_err(|_| failure("Fake event lock poisoned"))?;
        f.calls.push("reset");
        if f.fail_reset {
            f.fail_reset = false;
            return Err(failure("original reset cause"));
        }
        f.signalled = false;
        Ok(())
    }
    fn wait(
        &self,
        deadline: Option<Instant>,
        use_event: bool,
    ) -> Result<WaitOutcome, DesktopError> {
        let mut f = self.0.lock().map_err(|_| DesktopError::message("fake_event", "poison"))?;
        f.waits.push((deadline, use_event));
        if f.fail_wait {
            f.fail_wait = false;
            return Err(DesktopError::message("fake_wait", "original wait cause"));
        }
        Ok(if use_event && f.signalled { WaitOutcome::Woken } else { WaitOutcome::Deadline })
    }
    fn close(&self) -> Result<(), DesktopError> {
        let mut f = self.0.lock().map_err(|_| DesktopError::message("fake_event", "poison"))?;
        f.calls.push("close");
        if f.fail_close {
            f.fail_close = false;
            return Err(DesktopError::message("fake_close", "original close cause"));
        }
        f.closed = true;
        Ok(())
    }
}
#[derive(Default)]
struct Proxy {
    calls: AtomicUsize,
    fail: AtomicBool,
}
impl CompletionWake for Proxy {
    fn wake(&self) -> Result<(), ExecutionError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail.swap(false, Ordering::SeqCst) {
            return Err(failure("original proxy cause"));
        }
        Ok(())
    }
}
fn event() -> (Bootstrap, Arc<Event>) {
    let event = Arc::new(Event::default());
    (Bootstrap::from_event(event.clone()), event)
}
#[test]
fn arrivals_before_reset_during_drain_and_after_handoff_reach_the_retained_target() -> TestResult {
    let (bootstrap, event) = event();
    let endpoint = bootstrap.wake();
    let retained = endpoint.clone();
    endpoint.wake()?;
    bootstrap.begin_turn()?;
    assert!(!event.0.lock().map_err(|_| "lock")?.signalled);
    retained.wake()?;
    assert_eq!(bootstrap.wait(None)?, WaitOutcome::Woken);
    let proxy = Arc::new(Proxy::default());
    bootstrap.handoff(proxy.clone())?;
    retained.wake()?;
    assert_eq!(proxy.calls.load(Ordering::SeqCst), 2);
    let f = event.0.lock().map_err(|_| "lock")?;
    assert_eq!(f.calls, ["signal", "reset", "signal", "close"]);
    assert!(f.closed);
    drop(f);
    bootstrap.disconnect()?;
    Ok(())
}
#[test]
fn handoff_without_pending_event_still_schedules_one_native_drain() -> TestResult {
    let (bootstrap, event) = event();
    let proxy = Arc::new(Proxy::default());
    bootstrap.handoff(proxy.clone())?;
    assert_eq!(proxy.calls.load(Ordering::SeqCst), 1);
    assert_eq!(event.0.lock().map_err(|_| "lock")?.calls, ["close"]);
    bootstrap.disconnect()?;
    Ok(())
}
#[test]
fn reset_before_drain_keeps_a_new_arrival_signalled_until_the_next_drain() -> TestResult {
    let (bootstrap, event) = event();
    let endpoint = bootstrap.wake();
    endpoint.wake()?;
    bootstrap.begin_turn()?;
    endpoint.wake()?;
    assert!(event.0.lock().map_err(|_| "lock")?.signalled);
    bootstrap.begin_turn()?;
    assert!(!event.0.lock().map_err(|_| "lock")?.signalled);
    bootstrap.disconnect()?;
    Ok(())
}
#[test]
fn failed_proxy_publication_preserves_original_event_target_and_original_cause() -> TestResult {
    let (bootstrap, event) = event();
    let proxy = Arc::new(Proxy::default());
    proxy.fail.store(true, Ordering::SeqCst);
    let error = bootstrap.handoff(proxy.clone()).err().ok_or("Accepted failed proxy")?;
    assert!(error.to_string().contains("original proxy cause"));
    assert!(!event.0.lock().map_err(|_| "lock")?.closed);
    bootstrap.wake().wake()?;
    assert_eq!(event.0.lock().map_err(|_| "lock")?.calls, ["signal"]);
    bootstrap.handoff(proxy.clone())?;
    bootstrap.wake().wake()?;
    assert_eq!(proxy.calls.load(Ordering::SeqCst), 3);
    bootstrap.disconnect()?;
    Ok(())
}
#[test]
fn checked_close_failure_preserves_committed_proxy_and_retained_actual_event_custody() -> TestResult
{
    let (bootstrap, event) = event();
    event.0.lock().map_err(|_| "lock")?.fail_close = true;
    let proxy = Arc::new(Proxy::default());
    let error = bootstrap.handoff(proxy.clone()).err().ok_or("Accepted failed close")?;
    assert!(error.to_string().contains("original close cause"));
    assert!(!event.0.lock().map_err(|_| "lock")?.closed);
    bootstrap.wake().wake()?;
    assert_eq!(proxy.calls.load(Ordering::SeqCst), 2);
    bootstrap.disconnect()?;
    assert!(event.0.lock().map_err(|_| "lock")?.closed);
    let custody =
        error.source().and_then(|e| e.downcast_ref::<RetirementFailure>()).ok_or("Custody lost")?;
    assert!(Arc::ptr_eq(&custody.event, &(event.clone() as Arc<dyn EventBoundary>)));
    Ok(())
}
#[test]
fn returned_failed_closure_keeps_the_event_owner_independent_of_dropped_bootstrap() -> TestResult {
    let (bootstrap, event) = event();
    event.0.lock().map_err(|_| "lock")?.fail_close = true;
    let weak = Arc::downgrade(&event);
    let error = bootstrap.disconnect().err().ok_or("Failed closure accepted")?;
    drop(bootstrap);
    drop(event);
    assert!(weak.upgrade().is_some());
    let custody = error
        .source()
        .and_then(|e| e.downcast_ref::<RetirementFailure>())
        .ok_or("Retained event lost")?;
    custody.event.close()?;
    assert!(weak.upgrade().ok_or("owner disappeared")?.0.lock().map_err(|_| "lock")?.closed);
    drop(error);
    assert!(weak.upgrade().is_none());
    Ok(())
}
#[test]
fn disconnect_fences_all_clones_and_does_not_publish_to_a_closed_event_or_proxy() -> TestResult {
    let (bootstrap, event) = event();
    let endpoint = bootstrap.wake();
    let retained = endpoint.clone();
    let proxy = Arc::new(Proxy::default());
    bootstrap.handoff(proxy.clone())?;
    bootstrap.disconnect()?;
    bootstrap.disconnect()?;
    assert!(endpoint.wake().is_err());
    assert!(retained.wake().is_err());
    assert_eq!(proxy.calls.load(Ordering::SeqCst), 1);
    assert!(!event.0.lock().map_err(|_| "lock")?.calls.contains(&"signal"));
    Ok(())
}
#[test]
fn duplicate_handoff_and_handoff_after_disconnect_reject_before_native_output() -> TestResult {
    let (bootstrap, _) = event();
    let first = Arc::new(Proxy::default());
    let foreign = Arc::new(Proxy::default());
    bootstrap.handoff(first.clone())?;
    assert!(bootstrap.handoff(foreign.clone()).is_err());
    bootstrap.disconnect()?;
    assert!(bootstrap.handoff(foreign.clone()).is_err());
    assert_eq!(foreign.calls.load(Ordering::SeqCst), 0);
    assert_eq!(first.calls.load(Ordering::SeqCst), 1);
    Ok(())
}
#[test]
fn failed_reset_keeps_original_deadline_and_uses_message_only_closing_wait() -> TestResult {
    let (bootstrap, event) = event();
    bootstrap.wake().wake()?;
    event.0.lock().map_err(|_| "lock")?.fail_reset = true;
    assert!(bootstrap.begin_turn().is_err());
    let original = anchor() + std::time::Duration::from_secs(5);
    assert_eq!(bootstrap.wait(Some(original))?, WaitOutcome::Deadline);
    assert_eq!(event.0.lock().map_err(|_| "lock")?.waits, [(Some(original), false)]);
    bootstrap.begin_turn()?;
    bootstrap.disconnect()?;
    Ok(())
}
#[test]
fn failed_wait_switches_only_native_wait_input_without_renewing_deadline() -> TestResult {
    let (bootstrap, event) = event();
    event.0.lock().map_err(|_| "lock")?.fail_wait = true;
    let original = anchor();
    assert!(bootstrap.wait(Some(original)).is_err());
    assert_eq!(bootstrap.wait(Some(original))?, WaitOutcome::Deadline);
    assert_eq!(
        event.0.lock().map_err(|_| "lock")?.waits,
        [(Some(original), true), (Some(original), false)]
    );
    bootstrap.disconnect()?;
    Ok(())
}
#[test]
fn native_constructor_panic_retains_original_payload_for_owned_cleanup() -> TestResult {
    let error = construct_loop::<()>(|| {
        std::panic::resume_unwind(Box::new(String::from("original construction cause")))
    })
    .err()
    .ok_or("Constructor panic escaped")?;
    let cause = error
        .source()
        .and_then(|e| e.downcast_ref::<ConstructionFailure>())
        .ok_or("Original cause lost")?;
    assert_eq!(
        cause.0.downcast_ref::<String>().ok_or("Payload type lost")?,
        "original construction cause"
    );
    Ok(())
}

#[test]
fn failed_close_with_existing_application_error_retains_both_original_cause_and_actual_event()
-> TestResult {
    let (bootstrap, event) = event();
    event.0.lock().map_err(|_| "lock")?.fail_close = true;
    let weak = Arc::downgrade(&event);
    let mut failure =
        Some(DesktopError::message("actual_application", "original application cause"));
    bootstrap.retain_close_failure(&mut failure);
    drop(bootstrap);
    drop(event);
    let error = failure.ok_or("Failure lost")?;
    let retained =
        error.source().and_then(|e| e.downcast_ref::<RetainedCleanup>()).ok_or("Custody lost")?;
    assert_eq!(retained.primary.to_string(), "actual_application: original application cause");
    assert!(weak.upgrade().is_some());
    let custody = retained
        .retirement
        .source()
        .and_then(|e| e.downcast_ref::<RetirementFailure>())
        .ok_or("Event lost")?;
    custody.event.close()?;
    assert!(weak.upgrade().ok_or("Owner lost")?.0.lock().map_err(|_| "lock")?.closed);
    drop(error);
    assert!(weak.upgrade().is_none());
    Ok(())
}
