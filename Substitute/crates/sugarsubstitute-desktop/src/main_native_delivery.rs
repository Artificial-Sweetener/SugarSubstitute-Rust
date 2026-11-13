// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain typed panel delivery and original credit through actual native completion.

use crate::main_panel_ingress::{Lane, Ticket};
use crate::main_panel_ingress_receipt::ReceiptKind;
use crate::{
    CanvasNativeNotification, CanvasNativeReply, MainIdentity, PanelIngressReceipt, SurfaceReply,
};
use sugarsubstitute_contracts::execution::MemoryClaim;

/// Move the same owned response from application custody into a native callback slot.
pub(crate) enum NativeDelivery {
    Canvas(CanvasNativeReply),
    CanvasNotification(CanvasNativeNotification),
    Surface(SurfaceReply),
    Presentation(crate::SurfacePresentation),
    Ingress(PanelIngressReceipt),
    Memory(crate::MemoryCommand),
    Updates(crate::UpdatesCommand),
}

impl NativeDelivery {
    /// Borrow the navigation whose admitted allocation remains retained.
    pub(crate) fn identity(&self) -> MainIdentity {
        match self {
            Self::Canvas(value) => value.identity(),
            Self::CanvasNotification(value) => value.identity(),
            Self::Surface(value) => value.identity(),
            Self::Presentation(value) => value.identity(),
            Self::Ingress(value) => value.identity(),
            Self::Memory(value) => value.identity(),
            Self::Updates(value) => value.identity(),
        }
    }

    /// Select only the transport lane that owns this response or consumption receipt.
    pub(crate) fn lane(&self) -> Option<Lane> {
        match self {
            Self::Canvas(_) | Self::Surface(_) => Some(Lane::Ordinary),
            Self::CanvasNotification(_)
            | Self::Presentation(_)
            | Self::Memory(_)
            | Self::Updates(_) => None,
            Self::Ingress(value) => Some(match value.kind() {
                ReceiptKind::Control => Lane::Control,
                ReceiptKind::Cancellation { .. } => Lane::Cancellation,
            }),
        }
    }

    /// Preserve original correlation independently of the private transport ticket.
    pub(crate) fn request_id(&self) -> Option<u64> {
        match self {
            Self::Canvas(value) => Some(value.request_id()),
            Self::Surface(value) => Some(value.request_id()),
            Self::CanvasNotification(_)
            | Self::Presentation(_)
            | Self::Memory(_)
            | Self::Updates(_) => None,
            Self::Ingress(value) => match value.kind() {
                ReceiptKind::Control => None,
                ReceiptKind::Cancellation { request_id } => Some(request_id),
            },
        }
    }

    /// Borrow public response bytes; ingress receipts carry only private acknowledgement metadata.
    pub(crate) fn payload(&self) -> Option<&[u8]> {
        match self {
            Self::Canvas(value) => Some(value.payload()),
            Self::CanvasNotification(value) => Some(value.payload()),
            Self::Surface(value) => Some(value.payload()),
            Self::Presentation(value) => Some(value.payload()),
            Self::Memory(value) => Some(value.payload()),
            Self::Updates(value) => Some(value.payload()),
            Self::Ingress(_) => None,
        }
    }

    /// Move public response bytes to the bulk owner; receipts carry none.
    ///
    /// The delivery keeps its original allocation claim and transport correlation.
    pub(crate) fn take_payload(&mut self) -> Option<Vec<u8>> {
        match self {
            Self::Canvas(value) => Some(value.take_payload()),
            Self::CanvasNotification(value) => Some(value.take_payload()),
            Self::Surface(value) => Some(value.take_payload()),
            Self::Presentation(value) => Some(value.take_payload()),
            Self::Memory(value) => Some(value.take_payload()),
            Self::Updates(value) => Some(value.take_payload()),
            Self::Ingress(_) => None,
        }
    }

    /// Drop encoded storage before recovering its original noncloneable allocation claim.
    pub(crate) fn into_claim(self) -> MemoryClaim {
        match self {
            Self::Canvas(value) => value.into_claim(),
            Self::CanvasNotification(value) => value.into_claim(),
            Self::Surface(value) => value.into_claim(),
            Self::Presentation(value) => value.into_claim(),
            Self::Ingress(value) => value.into_claim(),
            Self::Memory(value) => value.into_claim(),
            Self::Updates(value) => value.into_claim(),
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
