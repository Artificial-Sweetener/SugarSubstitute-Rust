// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep fixed custody cutoff observations on the existing execution timer owner.
use sugarsubstitute_contracts::execution::Moment;
#[derive(Clone, Copy)]
struct Entry {
    until: Moment,
    due: bool,
    progress: Option<Moment>,
    progress_due: bool,
}
///64 actual backend cutoffs plus the helper's own close-only cutoff.
pub(crate) struct Deadlines {
    slots: [Option<Entry>; 65],
}
impl Deadlines {
    pub(crate) fn new() -> Self {
        Self { slots: [None; 65] }
    }
    pub(crate) fn arm(&mut self, index: usize, until: Moment) {
        if let Some(slot) = self.slots.get_mut(index)
            && slot.is_none()
        {
            *slot = Some(Entry { until, due: false, progress: None, progress_due: false });
        }
    }
    /// Schedule only one contention observation inside the unchanged cutoff.
    pub(crate) fn progress(&mut self, index: usize, at: Moment) -> bool {
        let Some(entry) = self.slots.get_mut(index).and_then(Option::as_mut) else {
            return false;
        };
        if entry.due || at > entry.until {
            return false;
        }
        if entry.progress.is_none() {
            entry.progress = Some(at);
            entry.progress_due = false;
        }
        true
    }
    /// Read a real consumed timer observation, never inferred native settlement.
    pub(crate) fn progress_due(&self, index: usize) -> bool {
        self.slots.get(index).and_then(Option::as_ref).is_some_and(|entry| entry.progress_due)
    }
    pub(crate) fn scan(&mut self, now: Moment) -> (Option<Moment>, bool) {
        let mut next = None;
        let mut changed = false;
        for entry in self.slots.iter_mut().flatten() {
            if entry.due {
                continue;
            }
            if now >= entry.until {
                entry.due = true;
                changed = true;
            } else {
                next = Some(next.map_or(entry.until, |old: Moment| old.min(entry.until)));
                if let Some(at) = entry.progress {
                    if now >= at {
                        entry.progress = None;
                        entry.progress_due = true;
                        changed = true;
                    } else {
                        next = Some(next.map_or(at, |old| old.min(at)));
                    }
                }
            }
        }
        (next, changed)
    }
    /// Remove only the last destroyed registration's original cutoff.
    pub(crate) fn disarm(&mut self, index: usize) {
        if let Some(slot) = self.slots.get_mut(index) {
            *slot = None;
        }
    }
    pub(crate) fn due(&self, index: usize) -> bool {
        self.slots.get(index).is_some_and(|entry| entry.is_some_and(|entry| entry.due))
    }
}
