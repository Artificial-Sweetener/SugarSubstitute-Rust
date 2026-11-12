// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Describe the exclusive native allocation boundary without releasing ownership through observations.

use super::cause::EntryEnvironmentCause;

/// Observe this boundary's local allocation state, not process or job settlement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AllocationState {
    /// No successful native getter has supplied an allocation.
    Vacant,
    /// The exact allocation is still available for bounded read and one Free.
    Held,
    /// A Free call failed or did not return a checked success; no read or retry.
    RetirementUncertain,
    /// Checked Free actually succeeded and returned.
    Released,
}

/// Own a stable provider before invoking Get, including across callback unwind.
/// Methods are synchronous finite boundary calls on the existing owned worker.
pub(crate) trait EnvironmentPort: Send {
    /// Adopt a non-null original allocation inside this provider before returning.
    /// A rejected second invocation performs no getter.
    fn get(&mut self) -> Result<(), EntryEnvironmentCause>;
    /// Read sequential original units, or re-read an already inspected prefix.
    /// The actual adapter guards pointer validity, double NUL and copied-unit bounds.
    fn read_unit(&mut self, index: usize) -> Result<u16, EntryEnvironmentCause>;
    /// Attempt checked Unicode retirement once, after all reader calls returned.
    /// Failure leaves opaque custody and prohibits another read or Free.
    fn free(&mut self) -> Result<(), EntryEnvironmentCause>;
    /// Observe retained local ownership without transferring an allocation.
    fn allocation_state(&self) -> AllocationState;
}
