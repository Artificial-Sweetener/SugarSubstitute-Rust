// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Project immutable startup input while the authenticated native map loan remains held.
use crate::{
    coordinator_entry::{EntryTransportError, startup_launch_payload},
    windows_lifecycle_pipe::StartupViewLoan,
};
use sugarsubstitute_contracts::coordinator_entry::EntryLaunchRequestView;

/// Retain one original authenticated read-only mapping loan for the desktop lifetime.
/// This input grants neither native capture authentication nor process authority.
/// Drop returns the loan; only the native worker may check physical map retirement.
pub struct AuthenticatedStartupSnapshot {
    loan: StartupViewLoan,
}
impl AuthenticatedStartupSnapshot {
    /// Called only after actual v3 kernel/capability/full-run authentication succeeds.
    pub(crate) fn from_authenticated_loan(loan: StartupViewLoan) -> Self {
        Self { loan }
    }
    /// Decode a borrowed view of the same bounded immutable Launch bytes.
    /// # Errors
    /// Returns the actual mapping or authoritative Launch-shape rejection.
    pub fn view(&self) -> Result<EntryLaunchRequestView<'_>, EntryTransportError> {
        startup_launch_payload::decode(self.loan.bytes()?)
    }
}
