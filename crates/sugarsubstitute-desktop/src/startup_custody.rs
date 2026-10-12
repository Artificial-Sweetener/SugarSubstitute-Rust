// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep startup's loop and STA custody until actual native retirement and execution settlement.

use crate::DesktopError;

/// Current owner observations; a native-loop return supplies none of these facts.
#[derive(Clone, Copy)]
pub(super) struct ShutdownFacts {
    pub(super) closing: bool,
    pub(super) splash_retired: bool,
    pub(super) main_retired: bool,
    pub(super) execution_settled: bool,
}
impl ShutdownFacts {
    fn settled(self) -> bool {
        self.closing && self.splash_retired && self.main_retired && self.execution_settled
    }
}

/// Own the single final teardown admission independently of native event-loop return status.
pub(super) struct LoopCustody {
    teardown_claimed: bool,
}
impl LoopCustody {
    pub(super) const fn new() -> Self {
        Self { teardown_claimed: false }
    }

    /// Request loop exit only after every retained owner actually settles.
    pub(super) fn may_exit(&self, facts: ShutdownFacts) -> bool {
        !self.teardown_claimed && facts.settled()
    }

    /// Check facts again after the loop returns, preserving an unexpected native status.
    /// An early return retains custody; its caller closes admission through the existing owner.
    pub(super) fn native_return(&mut self, facts: ShutdownFacts, exit_code: i32) -> NativeReturn {
        let ready = self.may_exit(facts);
        let failure = (!ready || exit_code != 0).then(|| {
            DesktopError::message(
                "startup_native_loop",
                format!("Native event loop returned without clean settlement (status {exit_code})"),
            )
        });
        let teardown = ready.then(|| {
            self.teardown_claimed = true;
            TeardownPermit(())
        });
        NativeReturn { failure, teardown }
    }
}

/// Return status and an independently established, single-use teardown proof.
pub(super) struct NativeReturn {
    pub(super) failure: Option<DesktopError>,
    pub(super) teardown: Option<TeardownPermit>,
}

/// A consumed proof allows the original loop, retired native holders and outer STA to release.
pub(super) struct TeardownPermit(());
impl TeardownPermit {
    /// Run final teardown once; this proof cannot be cloned or issued a second time.
    pub(super) fn finish<R>(self, teardown: impl FnOnce() -> R) -> R {
        teardown()
    }
}

#[cfg(test)]
#[path = "startup_custody_tests.rs"]
mod tests;
