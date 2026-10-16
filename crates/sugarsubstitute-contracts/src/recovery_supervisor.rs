// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Correlate a fresh subordinate with one retired run while the original coordinator survives.
//!
//! These are bounded matching facts. Native admission, authenticated peers, process
//! membership and durable acknowledgement remain independent adapter obligations.
mod identity;
pub mod wire;
pub use identity::*;
/// Bound fresh subordinate supervisors by the existing coordinator's run capacity.
pub const RECOVERY_SUPERVISOR_SLOTS: usize = 64;
/// Finite rejection without paths, tokens, wire bodies or native error text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryRejection {
    /// A positive grant or process identity was required.
    Identity,
    /// The source run, next generation or exact report correlation changed.
    Binding,
    /// This transition is premature, repeated or follows closed admission.
    Phase,
    /// An immutable original admission or shutdown interval expired.
    Deadline,
    /// Fixed source, frame or effect capacity was exceeded.
    Capacity,
}
impl std::fmt::Display for RecoveryRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Recovery supervisor rejected: {self:?}")
    }
}
impl std::error::Error for RecoveryRejection {}
/// First actual failure category; genuine native/provider sources stay with their owners.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryFailureCode {
    /// The actual owner cancelled before complete checked retirement.
    Cancelled,
    /// An original preparation or shutdown deadline expired.
    Deadline,
    /// The actual supervisor creation failed or stopped without an owned process.
    Launch,
    /// Kernel process, inherited capability, purpose or installed-image validation failed.
    Authentication,
    /// Fresh native registration or its owned handler failed.
    Capture,
    /// The actual Desktop creation failed or stopped without an owned process.
    Desktop,
    /// Authenticated original relay or framing failed.
    Transport,
    /// Actual family/native closure remained uncertain or failed.
    Retirement,
}
/// Current coverage is separate from old Root/client registrations and grants no capture authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryCaptureCoverage {
    /// Existing Root and previously attached windows remain without their lost handler.
    PriorRegistrationsLost,
    /// Only the fresh supervisor has registered; its Desktop is not yet capture-qualified.
    FreshSupervisorRegistered {
        /// Actual authenticated fresh supervisor identity.
        supervisor: RecoveryProcessIdentity,
        /// Actual successfully registered and independently watched handler identity.
        handler: RecoveryProcessIdentity,
    },
    /// Only this fresh subordinate and its newly attached Desktop family are registered.
    FreshFamilyRegistered {
        /// Actual authenticated fresh supervisor identity.
        supervisor: RecoveryProcessIdentity,
        /// Actual successfully registered and independently watched handler identity.
        handler: RecoveryProcessIdentity,
    },
}
