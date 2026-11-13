// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Reopen explicit installation choices before consulting optional update preparation.
use super::*;
use sugarsubstitute_supervision::setup_storage::InstalledCandidate;

impl Launcher {
    /// Read durable authorization and verify its runtime on the admitted storage worker.
    /// The returned creator retains setup custody until trial transfer or observed retirement.
    pub(super) fn reopen_installation(&mut self) -> Result<()> {
        let root = self.root.clone();
        self.submit(Task::Installation, move |context| {
            InstalledCandidate::reopen(&root, &context).map(|candidate| {
                Work::Installation(
                    candidate.map(|candidate| ApplicationLaunch::installed(Arc::new(candidate))),
                )
            })
        })
    }

    /// Completion records custody even during close; only the normal pump can admit a trial.
    /// Ordinary automatic preparation has no durable choice and keeps its live trust checks.
    pub(super) fn installed(&mut self, launch: Option<ApplicationLaunch>) {
        self.launch = launch;
        self.choice = if self.launch.is_some() {
            Choice::Trial
        } else if self.accepted_only {
            Choice::Accepted
        } else {
            Choice::Cache
        };
    }
}

#[cfg(test)]
mod tests;
