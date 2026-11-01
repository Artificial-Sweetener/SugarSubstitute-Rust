// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Bound scoped helper threads through which one CPU job spreads its borrowed work.

use std::{
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    sync::{
        Mutex, PoisonError,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
};
use sugarsubstitute_contracts::execution::{ForkJoin, ForkJoinItem};

/// Widest fork-join; display-sized work stops scaling with memory bandwidth beyond this.
const MAXIMUM_WIDTH: usize = 8;

/// Owner-wide helper budget shared by every CPU job's fork-join calls.
///
/// Helpers are scoped to one call, so each joins before `join` returns and none outlives
/// the job step. Calls that find the budget spent run their remaining items on the caller.
pub(crate) struct Helpers {
    width: usize,
    available: AtomicUsize,
}

impl Helpers {
    /// Size the width to half the logical processors, leaving room for the UI and webview.
    pub(crate) fn new(logical: usize) -> Self {
        let width = (logical / 2).clamp(1, MAXIMUM_WIDTH);
        Self { width, available: AtomicUsize::new(width - 1) }
    }

    fn acquire(&self) -> Option<Permit<'_>> {
        self.available
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |free| free.checked_sub(1))
            .ok()
            .map(|_| Permit(self))
    }
}

/// One admitted helper slot, returned to the budget when its thread finishes or never starts.
struct Permit<'a>(&'a Helpers);

impl Drop for Permit<'_> {
    fn drop(&mut self) {
        self.0.available.fetch_add(1, Ordering::AcqRel);
    }
}

/// Take and run one item unless another thread already took it.
fn run(slot: &Mutex<Option<ForkJoinItem<'_>>>) {
    let item = slot.lock().unwrap_or_else(PoisonError::into_inner).take();
    if let Some(item) = item {
        item();
    }
}

impl ForkJoin for Helpers {
    fn width(&self) -> usize {
        self.width
    }

    fn join<'a>(&self, items: Vec<ForkJoinItem<'a>>) {
        let slots: Vec<Mutex<Option<ForkJoinItem<'a>>>> =
            items.into_iter().map(|item| Mutex::new(Some(item))).collect();
        let mut panic = None;
        thread::scope(|scope| {
            for slot in slots.iter().skip(1) {
                let Some(permit) = self.acquire() else { break };
                let helper = thread::Builder::new()
                    .name("sugarsubstitute-cpu-helper".to_owned())
                    .spawn_scoped(scope, move || {
                        let _permit = permit;
                        run(slot);
                    });
                // A thread that could not start leaves its item for the caller.
                if helper.is_err() {
                    break;
                }
            }
            // The caller takes every item no helper has started, beginning with its own, and
            // keeps going past a panic so no item is left unrun.
            for slot in &slots {
                if let Err(payload) = catch_unwind(AssertUnwindSafe(|| run(slot))) {
                    panic.get_or_insert(payload);
                }
            }
        });
        if let Some(payload) = panic {
            resume_unwind(payload);
        }
    }
}

#[cfg(test)]
#[path = "fork_join_tests.rs"]
mod tests;
