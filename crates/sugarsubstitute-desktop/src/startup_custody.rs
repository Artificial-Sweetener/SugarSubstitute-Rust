// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain native teardown and final content custody until actual settlement and joins.

use crate::DesktopError;

/// Current owner observations; a native-loop return supplies none of these facts.
#[cfg(target_os = "windows")]
#[derive(Clone, Copy)]
pub(crate) struct ShutdownFacts {
    pub(crate) closing: bool,
    pub(crate) splash_retired: bool,
    pub(crate) main_retired: bool,
    pub(crate) execution_settled: bool,
}
#[cfg(target_os = "windows")]
impl ShutdownFacts {
    fn settled(self) -> bool {
        self.closing && self.splash_retired && self.main_retired && self.execution_settled
    }
}

/// Own the single final teardown admission independently of native event-loop return status.
#[cfg(target_os = "windows")]
pub(crate) struct LoopCustody {
    teardown_claimed: bool,
}
#[cfg(target_os = "windows")]
impl LoopCustody {
    pub(crate) const fn new() -> Self {
        Self { teardown_claimed: false }
    }

    /// Request loop exit only after every retained owner actually settles.
    pub(crate) fn may_exit(&self, facts: ShutdownFacts) -> bool {
        !self.teardown_claimed && facts.settled()
    }

    /// Check facts again after the loop returns, preserving an unexpected native status.
    /// An early return retains custody; its caller closes admission through the existing owner.
    pub(crate) fn native_return(&mut self, facts: ShutdownFacts, exit_code: i32) -> NativeReturn {
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
#[cfg(target_os = "windows")]
pub(crate) struct NativeReturn {
    pub(crate) failure: Option<DesktopError>,
    pub(crate) teardown: Option<TeardownPermit>,
}

/// A consumed native permit orders holder/loop drop before the actual final content join.
/// It grants no joined-worker fact and cannot by itself authorize releasing the STA.
#[cfg(target_os = "windows")]
pub(crate) struct TeardownPermit(());
#[cfg(target_os = "windows")]
impl TeardownPermit {
    /// Drop actual native holders/loop first, then finish settled workers while outer STA stays held.
    pub(crate) fn finish<N, R>(
        self,
        native: impl FnOnce() -> N,
        workers: impl FnOnce(N) -> R,
    ) -> R {
        let retired = native();
        workers(retired)
    }
}

/// Finish the consumed actual content owner only after native holders and loop retired.
///
/// Success disposes that finished owner before the final native release. Failure retains
/// one already-admitted owner without running Drop or granting a joined-worker fact.
/// This defensive path adds no task, queue, allocation or deadline: admission is closed
/// and its original buffers/claims/handles remain inaccessible until process termination.
/// The caller's acquired STA reference must remain unreleased on that path; its existing
/// apartment owner drops without CoUninitialize. Preserve the earliest original cause.
#[cfg(target_os = "windows")]
pub(crate) fn finish_after_native<C>(
    content: C,
    finish: impl FnOnce(&mut C) -> Result<(), DesktopError>,
    after_join: impl FnOnce() -> Result<(), DesktopError>,
    failure: &mut Option<DesktopError>,
) {
    finish_with_retention(content, finish, after_join, |_, cause| cause, failure);
}

/// Preserve application lifetime custody in a native failure before disposing content.
pub(crate) fn finish_window_content<C: crate::WindowContent>(
    content: C,
    after_join: impl FnOnce() -> Result<(), DesktopError>,
    failure: &mut Option<DesktopError>,
) {
    finish_with_retention(
        content,
        |content| {
            content.finish_shutdown().map_err(|cause| DesktopError::boxed("finish_shutdown", cause))
        },
        after_join,
        |content, cause| {
            DesktopError::boxed("native_surface_lifetime", content.retain_failure(cause))
        },
        failure,
    );
}

/// Attempt the final join before transferring original failure custody exactly once.
fn finish_with_retention<C>(
    mut content: C,
    finish: impl FnOnce(&mut C) -> Result<(), DesktopError>,
    after_join: impl FnOnce() -> Result<(), DesktopError>,
    retain: impl FnOnce(&mut C, DesktopError) -> DesktopError,
    failure: &mut Option<DesktopError>,
) {
    let joined = match finish(&mut content) {
        Ok(()) => true,
        Err(cause) => {
            tracing::error!(operation = "retain_final_content", cause = %cause,
            "Retaining one unresolved final content owner until process termination");
            DesktopError::retain_first(failure, cause);
            false
        }
    };
    if let Some(cause) = failure.take() {
        *failure = Some(retain(&mut content, cause));
    }
    if !joined {
        std::mem::forget(content);
        return;
    }
    drop(content);
    if let Err(cause) = after_join() {
        DesktopError::retain_first(failure, cause);
    }
}

#[cfg(all(test, target_os = "windows"))]
#[path = "startup_custody_tests.rs"]
mod tests;
