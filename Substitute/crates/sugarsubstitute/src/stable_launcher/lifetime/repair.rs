// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! When nothing installed can start, prepare the release again from the update feed and start
//! that, instead of ending the launch with a refusal.
//!
//! The accepted release may have lost a file (an antivirus quarantine, a disk fault), or the
//! installation may name nothing to start at all. The launcher then hosts the same preparation
//! worker an update check uses, in its repair mode, on its own executor: the feed's signatures,
//! release history and rollback protection apply unchanged, and what it prepares starts as an
//! ordinary trial. The launch's budget is replaced by the repair's own, and the waiting notice
//! stays up for as long as it takes. One launch repairs at most once.
use super::*;

impl Launcher {
    /// Begin the repair after selecting the accepted release failed with `cause`. `false` when
    /// this launch already repaired, when another copy or a busy record explains the failure,
    /// or when this build has no update feed; the caller then reports `cause`.
    ///
    /// # Errors
    /// Returns a malformed compiled feed or an unrepresentable budget.
    pub(super) fn begin_repair(&mut self, cause: &(dyn Error + 'static)) -> Result<bool> {
        if self.repair_started || self.accepted_only || held_elsewhere(cause) {
            return Ok(false);
        }
        let Some(worker) = crate::updates::repair_preparation(self.root.clone())? else {
            return Ok(false);
        };
        self.repair_started = true;
        tracing::warn!(
            operation = "startup_repair_begun",
            cause = %crate::failure_details::Details(cause),
            "Nothing installed can start; preparing the release again from the update feed"
        );
        self.extend_budget(LauncherBudget::repair(self.clock.now())?);
        self.cached = Some(cached::Cached::new(worker));
        self.choice = Choice::Cache;
        Ok(true)
    }
}

/// Whether the failure only says someone else holds the installation: another running copy, or
/// a record that stayed locked. Neither means the installed release is unusable.
fn held_elsewhere(cause: &(dyn Error + 'static)) -> bool {
    use sugarsubstitute_supervision::setup_storage::SelectionFailure;
    let mut next = Some(cause);
    while let Some(current) = next {
        // `io::Error::source` skips the wrapped error itself, so look inside it first.
        if let Some(inner) = current.downcast_ref::<io::Error>().and_then(io::Error::get_ref)
            && held_elsewhere(inner)
        {
            return true;
        }
        if matches!(
            current.downcast_ref::<SelectionFailure>(),
            Some(
                SelectionFailure::LockTimeout
                    | SelectionFailure::LockCancelled
                    | SelectionFailure::RunBusy(_)
                    | SelectionFailure::Superseded
                    | SelectionFailure::Changed
            )
        ) {
            return true;
        }
        next = current.source();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::held_elsewhere;
    use std::io;
    use sugarsubstitute_supervision::setup_storage::SelectionFailure;

    #[test]
    fn a_busy_installation_is_not_a_reason_to_repair_and_a_missing_release_is() {
        for busy in [
            SelectionFailure::LockTimeout,
            SelectionFailure::RunBusy(io::Error::other("held")),
            SelectionFailure::Superseded,
        ] {
            let wrapped = io::Error::other(busy);
            assert!(held_elsewhere(&wrapped));
        }
        let missing = io::Error::new(io::ErrorKind::NotFound, "sugarsubstitute.exe");
        assert!(!held_elsewhere(&missing));
        let nothing =
            io::Error::other("Installation has no accepted application; run setup to repair it");
        assert!(!held_elsewhere(&nothing));
    }
}
