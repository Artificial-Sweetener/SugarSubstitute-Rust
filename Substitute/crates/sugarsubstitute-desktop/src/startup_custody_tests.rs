// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Exercise production teardown admission with actual boundary facts, without native effects.

use super::*;

fn settled() -> ShutdownFacts {
    ShutdownFacts {
        closing: true,
        splash_retired: true,
        main_retired: true,
        execution_settled: true,
    }
}

#[test]
fn early_return_before_native_retirement_retains_loop_and_sta_custody() {
    let mut custody = LoopCustody::new();
    for facts in [
        ShutdownFacts { closing: false, ..settled() },
        ShutdownFacts { splash_retired: false, ..settled() },
        ShutdownFacts { main_retired: false, ..settled() },
        ShutdownFacts { splash_retired: false, main_retired: false, ..settled() },
    ] {
        assert!(!custody.may_exit(facts));
        let returned = custody.native_return(facts, 0);
        assert!(returned.failure.is_some());
        assert!(returned.teardown.is_none());
    }
    assert!(custody.may_exit(settled()));
}

#[test]
fn early_return_after_native_retirement_keeps_unsettled_workers_and_wake_owned() {
    let facts = ShutdownFacts { execution_settled: false, ..settled() };
    let mut custody = LoopCustody::new();
    assert!(!custody.may_exit(facts));
    for status in [0, 7, 0] {
        let returned = custody.native_return(facts, status);
        assert!(
            returned
                .failure
                .is_some_and(|cause| { cause.to_string().contains(&format!("status {status}")) })
        );
        assert!(returned.teardown.is_none());
    }
    assert!(custody.may_exit(settled()));
}

#[test]
fn actual_complete_settlement_admits_native_teardown_exactly_once()
-> Result<(), Box<dyn std::error::Error>> {
    let mut custody = LoopCustody::new();
    let returned = custody.native_return(settled(), 0);
    assert!(returned.failure.is_none());
    let mut releases = 0;
    returned
        .teardown
        .ok_or("actual settlement did not admit teardown")?
        .finish(|| releases += 1, |()| ());
    assert_eq!(releases, 1);
    assert!(!custody.may_exit(settled()));
    let duplicate = custody.native_return(settled(), 0);
    assert!(duplicate.failure.is_some());
    assert!(duplicate.teardown.is_none());
    assert_eq!(releases, 1);
    Ok(())
}

#[test]
fn failed_native_status_preserves_cause_while_actual_settlement_permits_teardown()
-> Result<(), Box<dyn std::error::Error>> {
    let mut custody = LoopCustody::new();
    let mut original = Some(DesktopError::message("original_owner", "initial failure"));
    let returned = custody.native_return(settled(), 9);
    DesktopError::retain_first(
        &mut original,
        returned.failure.ok_or("native status cause missing")?,
    );
    assert_eq!(
        original.ok_or("original owner cause missing")?.to_string(),
        "original_owner: initial failure"
    );
    let mut releases = 0;
    returned.teardown.ok_or("actual retirement proof missing")?.finish(|| releases += 1, |()| ());
    assert_eq!(releases, 1);
    Ok(())
}

#[test]
fn actual_native_holders_and_loop_release_before_worker_join_and_outer_sta_release()
-> Result<(), Box<dyn std::error::Error>> {
    use std::{cell::RefCell, rc::Rc};
    struct Holder(&'static str, Rc<RefCell<Vec<&'static str>>>);
    impl Drop for Holder {
        fn drop(&mut self) {
            self.1.borrow_mut().push(self.0);
        }
    }
    let order = Rc::new(RefCell::new(Vec::new()));
    let holders = [
        Holder("controls", order.clone()),
        Holder("presenter", order.clone()),
        Holder("window", order.clone()),
        Holder("loop", order.clone()),
    ];
    let mut custody = LoopCustody::new();
    let permit = custody.native_return(settled(), 0).teardown.ok_or("Teardown not admitted")?;
    permit.finish(
        || {
            order.borrow_mut().push("disconnect_wake");
            drop(holders);
        },
        |()| {
            assert_eq!(
                *order.borrow(),
                ["disconnect_wake", "controls", "presenter", "window", "loop"]
            );
            let mut failure = None;
            finish_after_native(
                Holder("content", order.clone()),
                |_| {
                    order.borrow_mut().push("join_workers");
                    Ok(())
                },
                || {
                    order.borrow_mut().push("release_sta");
                    Ok(())
                },
                &mut failure,
            );
            assert!(failure.is_none());
        },
    );
    assert_eq!(
        *order.borrow(),
        [
            "disconnect_wake",
            "controls",
            "presenter",
            "window",
            "loop",
            "join_workers",
            "content",
            "release_sta"
        ]
    );
    assert!(!custody.may_exit(settled()));
    Ok(())
}

#[test]
fn failed_final_join_retains_consumed_content_and_never_admits_release()
-> Result<(), Box<dyn std::error::Error>> {
    use std::{cell::Cell, rc::Rc};
    struct Content(Rc<Cell<usize>>);
    impl Drop for Content {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    for earlier in [false, true] {
        let drops = Rc::new(Cell::new(0));
        let finish_calls = Cell::new(0);
        let release_calls = Cell::new(0);
        let native_dropped = Cell::new(false);
        let mut failure =
            earlier.then(|| DesktopError::new("earlier", std::io::Error::other("original cause")));
        let mut custody = LoopCustody::new();
        custody.native_return(settled(), 0).teardown.ok_or("Teardown not admitted")?.finish(
            || native_dropped.set(true),
            |()| {
                finish_after_native(
                    Content(drops.clone()),
                    |_| {
                        assert!(native_dropped.get());
                        finish_calls.set(finish_calls.get() + 1);
                        Err(DesktopError::new("finish", std::io::Error::other("pre-join cause")))
                    },
                    || {
                        release_calls.set(release_calls.get() + 1);
                        Ok(())
                    },
                    &mut failure,
                )
            },
        );
        assert_eq!(finish_calls.get(), 1);
        assert_eq!(drops.get(), 0);
        assert_eq!(release_calls.get(), 0);
        assert!(!custody.may_exit(settled()));
        let cause = failure.ok_or("Original failure lost")?;
        assert_eq!(
            std::error::Error::source(&cause).ok_or("Cause source lost")?.to_string(),
            if earlier { "original cause" } else { "pre-join cause" }
        );
        drop(cause);
        assert_eq!(drops.get(), 0);
        assert_eq!(release_calls.get(), 0);
    }
    Ok(())
}

/// Preserve the original error allocation and final custody ordering on both join outcomes.
#[test]
fn failure_custody_transfers_after_join_attempt_before_content_drop_or_sta_release()
-> Result<(), Box<dyn std::error::Error>> {
    use std::{cell::RefCell, error::Error, rc::Rc};
    struct Content(Rc<RefCell<Vec<&'static str>>>);
    impl Drop for Content {
        fn drop(&mut self) {
            self.0.borrow_mut().push("drop");
        }
    }
    for joined in [false, true] {
        let order = Rc::new(RefCell::new(Vec::new()));
        let original = Box::new(std::io::Error::other("original allocation"));
        let identity = original.as_ref() as *const std::io::Error;
        let mut failure = Some(DesktopError::boxed("original", original));
        finish_with_retention(
            Content(order.clone()),
            |_| {
                order.borrow_mut().push("finish");
                if joined { Ok(()) } else { Err(DesktopError::message("finish", "unsettled")) }
            },
            || {
                order.borrow_mut().push("release");
                Ok(())
            },
            |_, cause| {
                order.borrow_mut().push("retain");
                assert_eq!(
                    cause
                        .source()
                        .and_then(|cause| cause.downcast_ref::<std::io::Error>())
                        .map(std::ptr::from_ref),
                    Some(identity)
                );
                DesktopError::new("custody", cause)
            },
            &mut failure,
        );
        assert_eq!(
            order.borrow().as_slice(),
            if joined {
                &["finish", "retain", "drop", "release"][..]
            } else {
                &["finish", "retain"][..]
            }
        );
        let retained = failure.ok_or("Failure custody lost")?;
        assert_eq!(
            retained
                .source()
                .and_then(Error::source)
                .and_then(|cause| cause.downcast_ref::<std::io::Error>())
                .map(std::ptr::from_ref),
            Some(identity)
        );
    }
    Ok(())
}

#[test]
fn failed_clock_cannot_suppress_closing_disposal_or_release_unsettled_custody()
-> Result<(), Box<dyn std::error::Error>> {
    use crate::timing::{
        NativeCallbackKind, NativeStartupOrder, QPC_SOURCE, Stamp, TimingAudit, TimingClock,
        TimingObserver,
    };
    use std::{cell::Cell, rc::Rc};
    struct Clock {
        failure: Rc<Cell<bool>>,
        reads: Rc<Cell<usize>>,
    }
    impl TimingClock for Clock {
        fn frequency(&self) -> i64 {
            1000
        }
        fn thread(&self) -> u32 {
            7
        }
        fn read(&mut self) -> Result<Stamp, DesktopError> {
            self.reads.set(self.reads.get() + 1);
            if self.failure.get() {
                return Err(DesktopError::new(
                    "dispatch_clock",
                    std::io::Error::other("clock unavailable"),
                ));
            }
            Ok(Stamp { ticks: 10, thread: 7, source: QPC_SOURCE })
        }
    }
    let failure = Rc::new(Cell::new(false));
    let reads = Rc::new(Cell::new(0));
    let observer = TimingObserver::new(TimingAudit::new(
        Box::new(Clock { failure: failure.clone(), reads: reads.clone() }),
        NativeStartupOrder::ExecutionFirst,
    )?);
    failure.set(true);
    let mut dispatched = 0;
    let original = crate::startup_runtime::observed_dispatch(
        Some(&observer),
        false,
        NativeCallbackKind::LoopProgress,
        || {
            dispatched += 1;
            Ok(())
        },
    )
    .err()
    .ok_or("Clock failure lost")?;
    assert_eq!(dispatched, 0);
    assert_eq!(reads.get(), 2);
    let mut retained = Some(original);
    let mut custody = LoopCustody::new();
    let mut facts = ShutdownFacts {
        closing: true,
        splash_retired: false,
        main_retired: false,
        execution_settled: false,
    };
    assert!(!custody.may_exit(facts));
    let cleanup = crate::startup_runtime::observed_dispatch(
        Some(&observer),
        true,
        NativeCallbackKind::LoopProgress,
        || {
            dispatched += 1;
            facts.splash_retired = true;
            Err(DesktopError::new("retire_main", std::io::Error::other("cleanup unavailable")))
        },
    )
    .err()
    .ok_or("Nested cleanup error swallowed")?;
    DesktopError::retain_first(&mut retained, cleanup);
    assert_eq!(reads.get(), 2);
    assert_eq!(dispatched, 1);
    assert!(!custody.may_exit(facts));
    crate::startup_runtime::observed_dispatch(
        Some(&observer),
        true,
        NativeCallbackKind::CompletionDrain,
        || {
            dispatched += 1;
            facts.main_retired = true;
            facts.execution_settled = true;
            Ok(())
        },
    )?;
    assert_eq!(reads.get(), 2);
    assert_eq!(dispatched, 2);
    assert!(custody.may_exit(facts));
    failure.set(false);
    let dispatch_failure = crate::startup_runtime::observed_dispatch(
        Some(&observer),
        false,
        NativeCallbackKind::CompletionDrain,
        || {
            failure.set(true);
            Err(DesktopError::new(
                "dispatch_native",
                std::io::Error::other("original callback cause"),
            ))
        },
    )
    .err()
    .ok_or("Original callback failure swallowed")?;
    assert_eq!(
        std::error::Error::source(&dispatch_failure).ok_or("Callback source lost")?.to_string(),
        "original callback cause"
    );
    assert_eq!(reads.get(), 4);
    let cause = retained.ok_or("Primary error lost")?;
    assert_eq!(
        std::error::Error::source(&cause).ok_or("Source lost")?.to_string(),
        "clock unavailable"
    );
    assert!(custody.native_return(facts, 0).teardown.is_some());
    Ok(())
}
