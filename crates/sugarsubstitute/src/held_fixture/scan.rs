// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Finish each bounded inventory pass without treating self-wakes as new native activity.
use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
use sugarsubstitute_contracts::execution::ExecutionError;

pub(super) struct ExternalWake {
    changed: AtomicBool,
    proxy: Arc<dyn CompletionWake>,
}
impl ExternalWake {
    pub(super) fn new(proxy: Arc<dyn CompletionWake>) -> Self {
        Self { changed: AtomicBool::new(false), proxy }
    }
    pub(super) fn take_changed(&self) -> bool {
        self.changed.swap(false, Ordering::AcqRel)
    }
}
impl CompletionWake for ExternalWake {
    fn wake(&self) -> std::result::Result<(), ExecutionError> {
        self.changed.store(true, Ordering::Release);
        self.proxy.wake()
    }
}

#[derive(Default)]
pub(super) struct Scan {
    remaining: usize,
    retry_after_pass: bool,
}
impl Scan {
    pub(super) fn request(&mut self) {
        self.remaining = HELD;
        self.retry_after_pass = false;
    }
    pub(super) fn begin(&mut self, external: bool, boundary_due: bool) {
        if external || (self.remaining == 0 && (boundary_due || self.retry_after_pass)) {
            self.request();
        }
    }
    pub(super) fn visit(&mut self) -> bool {
        if self.remaining == 0 {
            return false;
        }
        self.remaining -= 1;
        true
    }
    pub(super) fn retry_after_error(&mut self) {
        self.retry_after_pass = true;
    }
    pub(super) fn pending(&self) -> bool {
        self.remaining != 0 || self.retry_after_pass
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_off_cursor_native_milestone_is_reached_by_one_complete_bounded_pass() {
        let mut scan = Scan::default();
        let mut cursor = 10;
        let mut visited = [false; HELD];
        scan.begin(true, false);
        while scan.pending() {
            for _ in 0..5 {
                if !scan.visit() {
                    break;
                }
                visited[cursor] = true;
                cursor = (cursor + 1) % HELD;
            }
            scan.begin(false, false);
        }
        assert!(visited.into_iter().all(|seen| seen));
        assert_eq!(cursor, 10);
        assert!(!scan.visit());
    }
    #[test]
    fn self_wakes_finish_work_without_rearming_an_unchanged_inventory() {
        let mut scan = Scan::default();
        scan.request();
        let mut examined = 0;
        while scan.pending() {
            for _ in 0..5 {
                if scan.visit() {
                    examined += 1;
                }
            }
            scan.begin(false, false);
        }
        assert_eq!(examined, HELD);
        scan.begin(false, false);
        assert!(!scan.pending());
    }
    #[test]
    fn a_due_original_boundary_finishes_its_pass_before_considering_another() {
        let mut scan = Scan::default();
        scan.begin(false, true);
        for _ in 0..5 {
            assert!(scan.visit());
        }
        scan.begin(false, true);
        let mut examined = 5;
        while scan.pending() {
            for _ in 0..5 {
                if scan.visit() {
                    examined += 1;
                }
            }
            scan.begin(false, false);
        }
        assert_eq!(examined, HELD);
        scan.begin(false, false);
        assert!(!scan.pending());
    }

    #[test]
    fn an_early_error_keeps_unvisited_cases_and_a_following_cleanup_pass_reachable() {
        let mut scan = Scan::default();
        let mut cursor = 10;
        let mut visits = [0; HELD];
        scan.begin(true, false);
        assert!(scan.visit());
        visits[cursor] += 1;
        cursor = (cursor + 1) % HELD;
        scan.retry_after_error();
        while scan.pending() {
            scan.begin(false, false);
            for _ in 0..5 {
                if !scan.visit() {
                    break;
                }
                visits[cursor] += 1;
                cursor = (cursor + 1) % HELD;
            }
        }
        assert!(visits.into_iter().all(|count| count == 2));
        assert_eq!(cursor, 10);
    }
}
