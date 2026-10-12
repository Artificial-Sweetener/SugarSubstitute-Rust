// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Publish fixed command deadlines to the sole timer owner without exposing a scheduler.
use sugarsubstitute_contracts::execution::Moment;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Role {
    Handshake,
    Read,
    Write,
}
impl Role {
    pub(crate) const fn index(self) -> usize {
        match self {
            Self::Handshake => 0,
            Self::Read => 1,
            Self::Write => 2,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Armed,
    Due,
    Retiring,
}
#[derive(Clone, Copy, Debug)]
struct Entry {
    operation: u64,
    deadline: Moment,
    phase: Phase,
}
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Deadlines {
    entries: [Option<Entry>; 3],
    cleanup: [Option<Entry>; 2],
    stopped: bool,
}
impl Deadlines {
    pub(crate) fn handshake(deadline: Moment) -> Self {
        let mut value = Self::default();
        value.entries[0] = Some(Entry { operation: 0, deadline, phase: Phase::Armed });
        value
    }
    pub(crate) fn arm(&mut self, role: Role, operation: u64, deadline: Moment) -> bool {
        if self.stopped {
            return false;
        }
        let slot = &mut self.entries[role.index()];
        if slot.is_some() {
            return false;
        }
        *slot = Some(Entry { operation, deadline, phase: Phase::Armed });
        true
    }
    pub(crate) fn clear(&mut self, role: Role, operation: u64) -> bool {
        let slot = &mut self.entries[role.index()];
        if slot.is_some_and(|entry| entry.operation == operation) {
            *slot = None;
            true
        } else {
            false
        }
    }
    pub(crate) fn handshake_deadline(&self) -> Option<Moment> {
        self.entries[0].map(|entry| entry.deadline)
    }
    pub(crate) fn shorten_handshake(&mut self, deadline: Moment) -> bool {
        let Some(entry) = &mut self.entries[0] else {
            return false;
        };
        if entry.phase != Phase::Armed || deadline > entry.deadline {
            return false;
        }
        entry.deadline = deadline;
        true
    }
    pub(crate) fn scan(&mut self, now: Moment) -> (Option<Moment>, bool) {
        scan_entries(&mut self.entries, now)
    }
    pub(crate) fn scan_capture_cleanup(&mut self, now: Moment) -> (Option<Moment>, bool) {
        scan_entries(&mut self.cleanup, now)
    }
    pub(crate) fn expired(&self, now: Moment) -> bool {
        self.entries
            .iter()
            .flatten()
            .any(|entry| entry.phase != Phase::Retiring && entry.deadline <= now)
    }
    pub(crate) fn latch_capture_cleanup(
        &mut self,
        budget: sugarsubstitute_contracts::lifecycle::ShutdownBudget,
    ) {
        if self.cleanup.iter().any(Option::is_some) {
            return;
        }
        self.cleanup = [budget.graceful_until, budget.settle_until]
            .map(|deadline| Some(Entry { operation: 0, deadline, phase: Phase::Armed }));
    }
    pub(crate) fn capture_cleanup_due(&self, now: Moment) -> [bool; 2] {
        self.cleanup.map(|entry| entry.is_some_and(|entry| entry.deadline <= now))
    }
    pub(crate) fn stop(&mut self) {
        self.stopped = true;
        for entry in self.entries.iter_mut().flatten() {
            entry.phase = Phase::Retiring;
        }
    }
    pub(crate) fn stopped(&self) -> bool {
        self.stopped
    }
}

fn scan_entries(entries: &mut [Option<Entry>], now: Moment) -> (Option<Moment>, bool) {
    let mut future: Option<Moment> = None;
    let mut expired = false;
    for entry in entries.iter_mut().flatten() {
        if entry.phase != Phase::Armed {
            continue;
        }
        if entry.deadline <= now {
            entry.phase = Phase::Due;
            expired = true;
        } else {
            future = Some(future.map_or(entry.deadline, |earlier| earlier.min(entry.deadline)));
        }
    }
    (future, expired)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn due_entries_wake_once_and_stale_clear_cannot_remove_new_command() {
        let mut slots = Deadlines::handshake(Moment::from_nanos(20));
        assert!(slots.arm(Role::Read, 1, Moment::from_nanos(10)));
        assert!(!slots.arm(Role::Read, 2, Moment::from_nanos(5)));
        assert_eq!(slots.scan(Moment::from_nanos(0)), (Some(Moment::from_nanos(10)), false));
        assert_eq!(slots.scan(Moment::from_nanos(10)), (Some(Moment::from_nanos(20)), true));
        assert_eq!(slots.scan(Moment::from_nanos(10)), (Some(Moment::from_nanos(20)), false));
        assert!(slots.clear(Role::Read, 1));
        assert!(slots.arm(Role::Read, 2, Moment::from_nanos(30)));
        assert!(!slots.clear(Role::Read, 1));
        assert!(slots.clear(Role::Handshake, 0));
        assert_eq!(slots.scan(Moment::from_nanos(21)), (Some(Moment::from_nanos(30)), false));
        slots.stop();
        assert_eq!(slots.scan(Moment::from_nanos(100)), (None, false));
        assert!(!slots.expired(Moment::from_nanos(100)));
    }
}
