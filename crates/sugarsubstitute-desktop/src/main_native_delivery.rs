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
    Presentation(crate::SurfacePresentation),
    Ingress(PanelIngressReceipt),
    Memory(crate::MemoryCommand),
}

impl NativeDelivery {
    /// Borrow the navigation whose admitted allocation remains retained.
    pub(crate) fn identity(&self) -> MainIdentity {
        match self {
            Self::Context(value) => value.identity(),
            Self::Surface(value) => value.identity(),
            Self::Presentation(value) => value.identity(),
            Self::Ingress(value) => value.identity(),
            Self::Memory(value) => value.identity(),
        }
    }

    /// Select only the transport lane that owns this response or consumption receipt.
    pub(crate) fn lane(&self) -> Option<Lane> {
        match self {
            Self::Context(_) | Self::Surface(_) => Some(Lane::Ordinary),
            Self::Presentation(_) | Self::Memory(_) => None,
            Self::Ingress(value) => Some(match value.kind() {
                ReceiptKind::Control => Lane::Control,
                ReceiptKind::Cancellation { .. } => Lane::Cancellation,
            }),
        }
    }

    /// Preserve original correlation independently of the private transport ticket.
    pub(crate) fn request_id(&self) -> Option<u64> {
        match self {
            Self::Context(value) => Some(value.request_id()),
            Self::Surface(value) => Some(value.request_id()),
            Self::Presentation(_) | Self::Memory(_) => None,
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
            Self::Presentation(value) => Some(value.payload()),
            Self::Memory(value) => Some(value.payload()),
            Self::Ingress(_) => None,
        }
    }

    /// Drop encoded storage before recovering its original noncloneable allocation claim.
    pub(crate) fn into_claim(self) -> MemoryClaim {
        match self {
            Self::Context(value) => value.into_claim(),
            Self::Surface(value) => value.into_claim(),
            Self::Presentation(value) => value.into_claim(),
            Self::Ingress(value) => value.into_claim(),
            Self::Memory(value) => value.into_claim(),
        }
    }
}

/// Distinguish request-credit callbacks from independently admitted native observations.
pub(crate) enum DeliveryCorrelation {
    Transport(Ticket),
    Presentation,
}

/// Native command slots retain delivery and its precise completion ownership together.
pub(crate) struct OwnedDelivery {
    pub(crate) context: i64,
    pub(crate) correlation: DeliveryCorrelation,
    pub(crate) delivery: NativeDelivery,
}
