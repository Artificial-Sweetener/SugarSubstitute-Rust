// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain typed panel delivery and original credit through actual native completion.

use crate::main_panel_ingress::{Lane, Ticket};
use crate::main_panel_ingress_receipt::ReceiptKind;
use crate::{CanvasContextReply, MainIdentity, PanelIngressReceipt, SurfaceReply};
use sugarsubstitute_contracts::execution::MemoryClaim;

/// Move the same owned response from application custody into a native callback slot.
pub(crate) enum NativeDelivery {
    Context(CanvasContextReply),
    Surface(SurfaceReply),
    Ingress(PanelIngressReceipt),
}

impl NativeDelivery {
    /// Borrow the navigation whose admitted allocation remains retained.
    pub(crate) fn identity(&self) -> MainIdentity {
        match self {
            Self::Context(value) => value.identity(),
            Self::Surface(value) => value.identity(),
            Self::Ingress(value) => value.identity(),
        }
    }

    /// Select only the transport lane that owns this response or consumption receipt.
    pub(crate) fn lane(&self) -> Lane {
        match self {
            Self::Context(_) | Self::Surface(_) => Lane::Ordinary,
            Self::Ingress(value) => match value.kind() {
                ReceiptKind::Control => Lane::Control,
                ReceiptKind::Cancellation { .. } => Lane::Cancellation,
            },
        }
    }

    /// Preserve original correlation independently of the private transport ticket.
    pub(crate) fn request_id(&self) -> Option<u64> {
        match self {
            Self::Context(value) => Some(value.request_id()),
            Self::Surface(value) => Some(value.request_id()),
            Self::Ingress(value) => match value.kind() {
                ReceiptKind::Control => None,
                ReceiptKind::Cancellation { request_id } => Some(request_id),
            },
        }
    }

    /// Borrow public response bytes; ingress receipts carry only private acknowledgement metadata.
    pub(crate) fn payload(&self) -> Option<&[u8]> {
        match self {
            Self::Context(value) => Some(value.payload()),
            Self::Surface(value) => Some(value.payload()),
            Self::Ingress(_) => None,
        }
    }

    /// Drop encoded storage before recovering its original noncloneable allocation claim.
    pub(crate) fn into_claim(self) -> MemoryClaim {
        match self {
            Self::Context(value) => value.into_claim(),
            Self::Surface(value) => value.into_claim(),
            Self::Ingress(value) => value.into_claim(),
        }
    }
}

/// Native command slots retain delivery and its exact issued transport ticket together.
pub(crate) struct OwnedDelivery {
    pub(crate) context: i64,
    pub(crate) ticket: Ticket,
    pub(crate) delivery: NativeDelivery,
}
