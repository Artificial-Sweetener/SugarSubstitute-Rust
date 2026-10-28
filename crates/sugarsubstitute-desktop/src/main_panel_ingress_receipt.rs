// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain consumed control-lane allocations until their native completion is observed.

use crate::MainIdentity;
use std::{error::Error, fmt};
use sugarsubstitute_contracts::execution::MemoryClaim;

/// A consumed panel control or cancellation message and its original admitted storage.
///
/// Creating a receipt neither returns transport credit nor authorizes a native operation.
/// The desktop matches its complete navigation and cancellation identity before delivery.
pub struct PanelIngressReceipt {
    identity: MainIdentity,
    kind: ReceiptKind,
    claim: MemoryClaim,
}

/// Distinguish the two independently admitted control lanes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReceiptKind {
    /// A consumed panel handshake or connectivity observation.
    Control,
    /// A consumed cancellation for one exact canvas request.
    Cancellation { request_id: u64 },
}

/// Reject inadequate custody or a cancellation outside the public request identity range.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PanelIngressReceiptError {
    /// The original allocation cannot cover the bounded native delivery.
    Storage,
    /// The cancellation identity is not a positive protocol request identity.
    RequestIdentity,
}

impl PanelIngressReceipt {
    /// Original allocation retained through native submission and actual completion, in bytes.
    pub const STORAGE_BYTES: u64 = 64 * 1024;

    /// Retain a consumed control message without creating transport or navigation authority.
    /// # Errors
    /// Rejects a claim smaller than the required native delivery allocation.
    pub fn control(
        identity: MainIdentity,
        claim: MemoryClaim,
    ) -> Result<Self, PanelIngressReceiptError> {
        Self::new(identity, ReceiptKind::Control, claim)
    }

    /// Retain consumption of a cancellation for its exact original canvas request.
    /// # Errors
    /// Rejects inadequate storage or an identity outside the shared protocol's integer range.
    pub fn cancellation(
        identity: MainIdentity,
        request_id: u64,
        claim: MemoryClaim,
    ) -> Result<Self, PanelIngressReceiptError> {
        if !(1..=2147483647).contains(&request_id) {
            return Err(PanelIngressReceiptError::RequestIdentity);
        }
        Self::new(identity, ReceiptKind::Cancellation { request_id }, claim)
    }

    /// Preserve the same noncloneable ingress claim for either control lane.
    fn new(
        identity: MainIdentity,
        kind: ReceiptKind,
        claim: MemoryClaim,
    ) -> Result<Self, PanelIngressReceiptError> {
        if claim.bytes() < Self::STORAGE_BYTES {
            return Err(PanelIngressReceiptError::Storage);
        }
        Ok(Self { identity, kind, claim })
    }

    /// Return the complete originating navigation for exact retirement and reply correlation.
    /// This read-only identity grants no transport admission or native operation authority.
    pub const fn identity(&self) -> MainIdentity {
        self.identity
    }

    /// Return the consumed lane and its original cancellation correlation data.
    pub(crate) const fn kind(&self) -> ReceiptKind {
        self.kind
    }

    /// Transfer the original claim only after actual native completion and actor validation.
    #[cfg(any(target_os = "windows", test))]
    pub(crate) fn into_claim(self) -> MemoryClaim {
        self.claim
    }
}

impl fmt::Debug for PanelIngressReceipt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PanelIngressReceipt")
            .field("identity", &self.identity())
            .field("kind", &self.kind())
            .field("retained_bytes", &self.claim.bytes())
            .finish_non_exhaustive()
    }
}

impl fmt::Display for PanelIngressReceiptError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Storage => "Panel ingress receipt storage was not admitted",
            Self::RequestIdentity => "Panel cancellation request identity is invalid",
        })
    }
}

impl Error for PanelIngressReceiptError {}

#[cfg(test)]
mod tests;
