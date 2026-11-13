// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Fork-join runs every borrowed item once, inside the call, within the owner-wide budget.

use super::*;
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::atomic::AtomicU32,
    thread::ThreadId,
};

fn items<'a>(runs: &'a [AtomicU32]) -> Vec<ForkJoinItem<'a>> {
    runs.iter()
        .map(|count| {
            Box::new(move || {
                count.fetch_add(1, Ordering::SeqCst);
            }) as ForkJoinItem<'a>
        })
        .collect()
}

#[test]
fn width_is_half_the_processors_within_bounds() {
    assert_eq!(Helpers::new(0).width(), 1);
    assert_eq!(Helpers::new(1).width(), 1);
    assert_eq!(Helpers::new(4).width(), 2);
    assert_eq!(Helpers::new(32).width(), MAXIMUM_WIDTH);
}

#[test]
fn every_item_runs_exactly_once_before_join_returns_and_permits_return() {
    let helpers = Helpers::new(32);
    let runs: Vec<AtomicU32> = (0..23).map(|_| AtomicU32::new(0)).collect();
    helpers.join(items(&runs));
    assert!(runs.iter().all(|count| count.load(Ordering::SeqCst) == 1));
    assert_eq!(helpers.available.load(Ordering::SeqCst), MAXIMUM_WIDTH - 1);
}

#[test]
fn a_spent_budget_runs_every_item_on_the_caller() {
    let helpers = Helpers::new(32);
    let held: Vec<Permit<'_>> = std::iter::from_fn(|| helpers.acquire()).collect();
    assert_eq!(held.len(), MAXIMUM_WIDTH - 1);
    let caller = thread::current().id();
    let threads: Vec<Mutex<Option<ThreadId>>> = (0..5).map(|_| Mutex::new(None)).collect();
    let work = threads
        .iter()
        .map(|slot| {
            Box::new(move || {
                *slot.lock().unwrap_or_else(PoisonError::into_inner) = Some(thread::current().id());
            }) as ForkJoinItem<'_>
        })
        .collect();
    helpers.join(work);
    for slot in &threads {
        assert_eq!(*slot.lock().unwrap_or_else(PoisonError::into_inner), Some(caller));
    }
    drop(held);
    assert_eq!(helpers.available.load(Ordering::SeqCst), MAXIMUM_WIDTH - 1);
}

#[test]
fn a_panicking_item_resumes_on_the_caller_after_the_others_finish() {
    let helpers = Helpers::new(8);
    let runs: Vec<AtomicU32> = (0..6).map(|_| AtomicU32::new(0)).collect();
    let mut work = items(&runs);
    work.insert(2, Box::new(|| std::panic::resume_unwind(Box::new("item failed"))));
    let outcome = catch_unwind(AssertUnwindSafe(|| helpers.join(work)));
    assert!(outcome.is_err());
    assert!(runs.iter().all(|count| count.load(Ordering::SeqCst) == 1));
    assert_eq!(helpers.available.load(Ordering::SeqCst), helpers.width() - 1);
}
