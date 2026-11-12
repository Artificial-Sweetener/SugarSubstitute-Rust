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
    capture: bool,
}

///64 actual backend cutoffs plus the helper's own close-only cutoff.
pub(crate) struct Deadlines {
    slots: [Option<Entry>; 65],
    parent: Option<Moment>,
}
impl Deadlines {
    pub(crate) fn new() -> Self {
        Self { slots: [None; 65], parent: None }
    }
    /// Replace only the renewable parent observation; cleanup cutoffs stay immutable.
    pub(crate) fn observe_parent_at(&mut self, at: Option<Moment>) -> bool {
        if self.parent.is_some() && at.is_some() {
            return false;
        }
        let changed = self.parent != at;
        self.parent = at;
        changed
    }
    pub(crate) fn arm(&mut self, index: usize, until: Moment) {
        if let Some(slot) = self.slots.get_mut(index)
            && slot.is_none()
        {
            *slot = Some(Entry {
                until,
                due: false,
                progress: None,
                progress_due: false,
                capture: false,
            });
        }
    }
    /// Arm only the original recovery capture stage on an unused timer cell.
    pub(crate) fn capture(&mut self, index: usize, until: Moment) -> bool {
        let Some(slot) = self.slots.get_mut(index) else {
            return false;
        };
        if slot.is_some() {
            return false;
        }
        *slot =
            Some(Entry { until, due: false, progress: None, progress_due: false, capture: true });
        true
    }
    /// Consume the one capture stage after its actual cutoff, then freeze containment.
    pub(crate) fn after_capture(&mut self, index: usize, now: Moment, until: Moment) -> bool {
        let Some(slot) = self.slots.get_mut(index) else {
            return false;
        };
        let Some(entry) = slot.as_ref() else {
            return false;
        };
        if !entry.capture || now < entry.until || until <= entry.until {
            return false;
        }
        *slot = Some(Entry {
            until,
            due: now >= until,
            progress: None,
            progress_due: false,
            capture: false,
        });
        true
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
        if let Some(at) = self.parent {
            if now >= at {
                self.parent = None;
                changed = true;
            } else {
                next = Some(at);
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_worker_progress_cannot_postpone_parent_observation() {
        let mut deadlines = Deadlines::new();
        assert!(deadlines.observe_parent_at(Some(Moment::from_nanos(10))));
        assert!(!deadlines.observe_parent_at(Some(Moment::from_nanos(20))));
        assert_eq!(deadlines.scan(Moment::from_nanos(9)), (Some(Moment::from_nanos(10)), false));
        assert_eq!(deadlines.scan(Moment::from_nanos(10)), (None, true));
        assert!(deadlines.observe_parent_at(Some(Moment::from_nanos(20))));
        assert_eq!(deadlines.scan(Moment::from_nanos(20)), (None, true));
    }
    #[test]
    fn parent_observation_never_renews_or_removes_owned_cleanup() {
        let mut deadlines = Deadlines::new();
        deadlines.arm(64, Moment::from_nanos(15));
        deadlines.observe_parent_at(Some(Moment::from_nanos(10)));
        assert_eq!(deadlines.scan(Moment::from_nanos(10)), (Some(Moment::from_nanos(15)), true));
        deadlines.observe_parent_at(Some(Moment::from_nanos(20)));
        assert!(deadlines.observe_parent_at(None));
        assert_eq!(deadlines.scan(Moment::from_nanos(15)), (None, true));
        assert!(deadlines.due(64));
    }
}
