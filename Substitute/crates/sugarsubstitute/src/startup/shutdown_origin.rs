// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Distinguish unadmitted capture, pending measurement and the original observed budget.
use sugarsubstitute_contracts::lifecycle::ShutdownBudget;
/// Preserve pending capture clock observation separately from an absent capture owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShutdownInitiation {
    /// No capture session was admitted; the composing owner measures its own clock once.
    NoCapture,
    /// The existing capture claimant is still observing its original clock.
    CapturePending,
    /// The genuine capture owner supplied its unchanged ten/fifteen-second transaction.
    Captured(ShutdownBudget),
}
