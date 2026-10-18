// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain only the sole desktop retirement producer's completed run identity.
use crate::production::parent_lifecycle::RetiredRegistration;
use sugarsubstitute_contracts::lifecycle::RunKey;
/// Same-run receipt emitted by the native desktop retirement owner after checked cleanup.
/// Matching identities, exit codes and cancelled work cannot construct this value.
pub(crate) struct RetiredDesktop {
    registration: RetiredRegistration,
}
impl RetiredDesktop {
    /// Substitute the native retirement boundary solely for closed-role classification tests.
    #[cfg(test)]
    pub(in crate::production) fn boundary_fixture(run: RunKey, registered: bool) -> Self {
        Self { registration: RetiredRegistration::boundary_fixture(run, registered) }
    }
    pub(super) fn new(registration: RetiredRegistration) -> Self {
        Self { registration }
    }
    /// Return the exact already-retired run as inert matching data.
    pub(crate) fn run(&self) -> RunKey {
        self.registration.run()
    }
}

impl RetiredDesktop {
    /// Observe the actual parent admission disposition after checked native cleanup.
    pub(crate) fn registered(&self) -> bool {
        self.registration.registered()
    }
}
