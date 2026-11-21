// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Say once, in the operational log, when pointer input stops reaching the browser.
//!
//! A page that takes no clicks at all looks frozen to the person using it. The log then shows
//! whether the host refused the input, and why: the window was inactive, or the event fence was
//! no longer accepting. Each run of refusals is logged once, and admission ends the run.

use std::cell::Cell;

/// One run of refused input at a time, so a stuck window does not flood the log.
#[derive(Default)]
pub(super) struct RefusalLog {
    logged: Cell<bool>,
}

impl RefusalLog {
    /// Admit input only while the window is active and accepting, logging a refusal once.
    pub(super) fn admit(&self, active: bool, accepting: bool) -> bool {
        let admitted = active && accepting;
        if admitted {
            self.logged.set(false);
        } else if !self.logged.replace(true) {
            tracing::warn!(
                operation = "browser_input_refused",
                active,
                accepting,
                "Pointer input is not reaching the browser"
            );
        }
        admitted
    }
}

#[cfg(test)]
mod tests {
    use super::RefusalLog;

    #[test]
    fn a_run_of_refusals_is_logged_once_and_admission_ends_it() {
        let log = RefusalLog::default();
        assert!(!log.admit(false, true));
        assert!(log.logged.get());
        assert!(!log.admit(false, true));
        assert!(log.logged.get());
        assert!(log.admit(true, true));
        assert!(!log.logged.get());
    }
}
