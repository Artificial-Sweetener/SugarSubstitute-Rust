// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain admitted limited-host replies and independent ingress receipts until native delivery.

use super::*;
use sugarsubstitute_contracts::{
    canvas_native::{CanvasFailure, CanvasNativeRequest},
    canvas_surface::{
        CanvasSurfaceCapabilities, CanvasSurfaceDiscoveryStatus, CanvasSurfaceMessage,
        CanvasSurfaceReplyStatus,
    },
};
use sugarsubstitute_desktop::{CanvasNativeReply, MainHosting, PanelIngressReceipt, SurfaceReply};

enum Ordinary {
    Native(CanvasNativeReply),
    Surface(SurfaceReply),
}

#[derive(Clone, Copy)]
enum Lane {
    Ordinary,
    Control,
    Cancellation,
}

impl Lane {
    const fn next(self) -> Self {
        match self {
            Self::Ordinary => Self::Control,
            Self::Control => Self::Cancellation,
            Self::Cancellation => Self::Ordinary,
        }
    }
}

/// Mirror the native ordinary/control/cancellation lanes without granting additional admission.
pub(in crate::production::child) struct Deliveries {
    binding: Option<(MainIdentity, MemoryLease)>,
    ordinary: Option<Ordinary>,
    control: Option<PanelIngressReceipt>,
    cancellation: Option<PanelIngressReceipt>,
    next: Lane,
}

impl Deliveries {
    /// Begin without a native navigation or retained ingress allocation.
    pub(in crate::production::child) const fn new() -> Self {
        Self {
            binding: None,
            ordinary: None,
            control: None,
            cancellation: None,
            next: Lane::Ordinary,
        }
    }

    /// Bind only the caller's already-attributed native identity and original reservation.
    /// A changed lifetime requires its owner's explicit retirement fence and clear.
    pub(in crate::production::child) fn bind(
        &mut self,
        identity: MainIdentity,
        lease: &MemoryLease,
    ) -> Result<()> {
        match &self.binding {
            Some((current, original))
                if *current != identity || !original.same_reservation(lease) =>
            {
                Err("Panel delivery belongs to another native lifetime".into())
            }
            Some(_) => Ok(()),
            None => {
                self.binding = Some((identity, lease.clone()));
                Ok(())
            }
        }
    }

    /// Release queued native allocations after their navigation's ingress ledger is fenced.
    pub(in crate::production::child) fn clear(&mut self) {
        self.ordinary = None;
        self.control = None;
        self.cancellation = None;
        self.binding = None;
        self.next = Lane::Ordinary;
    }

    /// Reject a second unreturned control before synchronous policy processing can change state.
    pub(super) fn check_control(&self) -> Result<()> {
        if self.control.is_some() {
            return Err("Panel control receipt slot is occupied".into());
        }
        Ok(())
    }

    /// Retain the consumed control's original claim independently of ordinary response credit.
    pub(super) fn complete_control(&mut self, receipt: PanelIngressReceipt) -> Result<()> {
        if self.binding.as_ref().is_none_or(|(identity, _)| *identity != receipt.identity()) {
            return Err("Panel control receipt belongs to another native lifetime".into());
        }
        self.check_control()?;
        self.control = Some(receipt);
        Ok(())
    }

    /// Refuse a surface operation the native surface cannot answer yet, or acknowledge
    /// consumption of a cancellation.
    ///
    /// A composition-hosted browser was told the host presents its output, so it hears that the
    /// surface is unavailable for now; a windowed browser has no native surface at all, so it
    /// hears that the surface is unsupported.
    pub(super) fn surface(
        &mut self,
        identity: MainIdentity,
        message: CanvasSurfaceMessage,
        lease: &MemoryLease,
        claim: MemoryClaim,
        hosting: MainHosting,
    ) -> Result<()> {
        self.bind(identity, lease)?;
        if let CanvasSurfaceMessage::Cancel { session, request_id } = message {
            if session != PanelSession::for_main(identity)? {
                return Err("Panel cancellation belongs to another native session".into());
            }
            if self.cancellation.is_some() {
                return Err("Panel cancellation receipt slot is occupied".into());
            }
            self.cancellation =
                Some(PanelIngressReceipt::cancellation(identity, request_id, claim)?);
            return Ok(());
        }
        self.check_ordinary()?;
        let absent = if hosting.presents_native_canvas() {
            CanvasSurfaceDiscoveryStatus::Unavailable
        } else {
            CanvasSurfaceDiscoveryStatus::Unsupported
        };
        let reply = match message {
            CanvasSurfaceMessage::Request(request) => SurfaceReply::new(
                identity,
                &request,
                CanvasSurfaceReplyStatus::Unavailable,
                "",
                claim,
            )?,
            CanvasSurfaceMessage::Discovery(request) => SurfaceReply::discovery(
                identity,
                &request,
                absent,
                &CanvasSurfaceCapabilities::default(),
                "",
                claim,
            )?,
            CanvasSurfaceMessage::Presentation(request) => {
                SurfaceReply::presentation(identity, &request, absent, false, "", claim)?
            }
            CanvasSurfaceMessage::Cancel { .. } => {
                return Err("Panel cancellation escaped its dedicated delivery lane".into());
            }
        };
        self.ordinary = Some(Ordinary::Surface(reply));
        Ok(())
    }

    /// Answer a native canvas request through its original ingress claim, even before panel
    /// readiness or after failure: an empty context, or `unavailable` for every other family.
    pub(super) fn native(
        &mut self,
        identity: MainIdentity,
        request: &CanvasNativeRequest,
        lease: &MemoryLease,
        claim: MemoryClaim,
        unavailable: &CanvasFailure,
    ) -> Result<()> {
        self.bind(identity, lease)?;
        self.check_ordinary()?;
        let reply = CanvasNativeReply::new(identity, request.refusal_reply(unavailable)?, claim)?;
        self.ordinary = Some(Ordinary::Native(reply));
        Ok(())
    }

    fn check_ordinary(&self) -> Result<()> {
        if self.ordinary.is_some() {
            return Err("Panel ordinary response slot is occupied".into());
        }
        Ok(())
    }

    /// Deliver only to the still-current navigation; lifecycle commands retain their own priority.
    pub(in crate::production::child) fn take_command(
        &mut self,
        current: MainIdentity,
    ) -> Option<DesktopStartupCommand> {
        if self.binding.as_ref().is_some_and(|(identity, _)| *identity != current) {
            self.clear();
            return None;
        }
        for _ in 0..3 {
            let lane = self.next;
            self.next = lane.next();
            let command = match lane {
                Lane::Ordinary => self.ordinary.take().map(|reply| match reply {
                    Ordinary::Native(reply) => DesktopStartupCommand::SendCanvasNative(reply),
                    Ordinary::Surface(reply) => DesktopStartupCommand::SendSurface(reply),
                }),
                Lane::Control => {
                    self.control.take().map(DesktopStartupCommand::CompletePanelIngress)
                }
                Lane::Cancellation => {
                    self.cancellation.take().map(DesktopStartupCommand::CompletePanelIngress)
                }
            };
            if command.is_some() {
                return command;
            }
        }
        None
    }
}

#[cfg(test)]
#[path = "deliveries/tests.rs"]
mod tests;
