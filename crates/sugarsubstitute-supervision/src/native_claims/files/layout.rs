// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Expose inert source-owned native layout facts to the closed diagnostic ledger.
use std::{error::Error, fmt};

/// Actual retained and peak storage facts for one fixed source-owned role.
/// Grant factories choose a named owner provider; this value grants no allocation.
pub(crate) struct NativeRoleLayout {
    /// Storage which remains live after the producing phase returns, in bytes.
    pub(crate) retained: usize,
    /// Complete simultaneous storage for that role, in bytes.
    pub(crate) peak: usize,
}
impl NativeRoleLayout {
    /// Return retained storage in bytes, including its actual owner controls.
    pub(crate) const fn retained_bytes(&self) -> usize {
        self.retained
    }
    /// Return simultaneous storage in bytes before the role allocates or effects.
    pub(crate) const fn peak_bytes(&self) -> usize {
        self.peak
    }
}
/// Reject a fixed source-owned layout before construction or native effects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeLayoutFailure {
    /// Checked size or alignment arithmetic could not represent the layout.
    Overflow,
}
impl fmt::Display for NativeLayoutFailure {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        output.write_str("Native owner layout exceeds its representation")
    }
}
impl Error for NativeLayoutFailure {}
