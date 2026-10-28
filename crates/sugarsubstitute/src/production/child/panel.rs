// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose panel handshake decisions with the current native document and its memory admission.

use super::*;
use std::time::Duration;
use sugarsubstitute_application::{
    panel::{PanelEffects, PanelHostSession, PanelState},
    startup::StartupPhase,
};
use sugarsubstitute_contracts::canvas_context::{CanvasContextRequest, CanvasDisplayCapability};
use sugarsubstitute_contracts::canvas_surface::{
    CanvasSurfaceCapabilities, CanvasSurfaceDiscoveryStatus, CanvasSurfaceMessage,
    CanvasSurfaceReplyStatus,
};
use sugarsubstitute_contracts::panel::{
    PanelMessage, PanelPalette, PanelPresentation, PanelSession, PanelStyle,
};
use sugarsubstitute_desktop::{
    CanvasContextObservation, CanvasContextReply, PanelCommand, PanelObservation,
    SurfaceObservation, SurfaceReply,
};

const OWNER_BYTES: u64 = 4096;
const ADMISSION_RETRY: Duration = Duration::from_millis(100);

enum CanvasReply {
    Context(CanvasContextReply),
    Surface(SurfaceReply),
}

/// Retain one current page's policy and at most one pending message of each host kind.
pub(super) struct PanelOwner {
    identity: MainIdentity,
    policy: PanelHostSession,
    lease: MemoryLease,
    configuration: Option<PanelCommand>,
    ping: Option<PanelCommand>,
    canvas_replies: [Option<CanvasReply>; 4],
    pending_applied: bool,
    deadline: Option<Moment>,
    admission_warned: bool,
    _claim: MemoryClaim,
}

impl PanelOwner {
    /// Check the current navigation and its original admitted native allocation.
    #[cfg(feature = "native-composition")]
    pub(super) fn admits_canvas(&self, identity: MainIdentity, lease: &MemoryLease) -> bool {
        self.identity == identity
            && self.lease.same_reservation(lease)
            && self.policy.state() != PanelState::Failed
    }

    /// Claim the persistent state before constructing the navigation-bound panel policy.
    fn new(identity: MainIdentity, lease: MemoryLease, now: Moment) -> Result<Option<Self>> {
        let claim = match lease.claim(OWNER_BYTES) {
            Ok(claim) => claim,
            Err(_) => return Ok(None),
        };
        let session = PanelSession::for_main(identity)?;
        let presentation =
            PanelPresentation::new(false, PanelStyle::Comfy, PanelPalette::DarkGray, None, 0)?;
        let policy = PanelHostSession::new(session, presentation, now)?;
        let deadline = policy.next_deadline()?;
        Ok(Some(Self {
            identity,
            policy,
            lease,
            configuration: None,
            ping: None,
            canvas_replies: std::array::from_fn(|_| None),
            pending_applied: false,
            deadline,
            admission_warned: false,
            _claim: claim,
        }))
    }

    /// Advance one admitted client message; repeat configuration is coalesced while queued.
    fn receive(&mut self, message: PanelMessage, now: Moment) -> Result<()> {
        if matches!(message, PanelMessage::Ready(_)) && self.configuration.is_some() {
            return Ok(());
        }
        let pending_deadline = self.pending_applied.then_some(self.deadline).flatten();
        let applied = matches!(message, PanelMessage::Applied(_));
        let claim = if matches!(message, PanelMessage::Ready(_) | PanelMessage::Applied(_)) {
            let Some(claim) = self.command_claim() else {
                if applied {
                    self.pending_applied = true;
                    self.schedule_pending(now)?;
                }
                return Ok(());
            };
            Some(claim)
        } else {
            None
        };
        let effects = self.policy.receive(message, now)?;
        if applied {
            self.pending_applied = false;
        }
        self.adopt(effects, claim)?;
        self.deadline = self.policy.next_deadline()?;
        if self.pending_applied {
            if let Some(previous) = pending_deadline {
                self.deadline =
                    Some(self.deadline.map_or(previous, |current| current.min(previous)));
            }
            self.schedule_pending(now)?;
        }
        Ok(())
    }

    /// Advance only an actual due deadline without manufacturing a timer or task.
    fn tick(&mut self, now: Moment) -> Result<()> {
        if self.deadline.is_none_or(|deadline| now < deadline) {
            return Ok(());
        }
        let Some(claim) = self.command_claim() else {
            let effects = self.policy.expire(now)?;
            if self.policy.state() == PanelState::Failed {
                self.pending_applied = false;
            }
            self.adopt(effects, None)?;
            self.deadline = if self.policy.state() == PanelState::Failed {
                None
            } else {
                let retry = now.checked_add(ADMISSION_RETRY)?;
                Some(self.policy.next_expiration()?.map_or(retry, |cutoff| cutoff.min(retry)))
            };
            return Ok(());
        };
        let semantic_due = self.policy.next_deadline()?.is_some_and(|deadline| now >= deadline);
        let effects = if self.pending_applied && !semantic_due {
            self.pending_applied = false;
            self.policy
                .receive(PanelMessage::Applied(PanelSession::for_main(self.identity)?), now)?
        } else {
            let effects = self.policy.tick(now)?;
            if self.policy.state() == PanelState::Failed {
                self.pending_applied = false;
            }
            effects
        };
        self.adopt(effects, Some(claim))?;
        self.deadline = self.policy.next_deadline()?;
        if self.pending_applied {
            self.schedule_pending(now)?;
        }
        Ok(())
    }

    /// Retry the one coalesced acknowledgement without renewing the policy's own deadline.
    fn schedule_pending(&mut self, now: Moment) -> Result<()> {
        let retry = now.checked_add(ADMISSION_RETRY)?;
        self.deadline = Some(self.deadline.map_or(retry, |deadline| deadline.min(retry)));
        Ok(())
    }

    /// Retain one generated command per kind before returning to the native pump.
    fn adopt(&mut self, effects: PanelEffects, claim: Option<MemoryClaim>) -> Result<()> {
        if effects.state == Some(PanelState::Failed) {
            self.configuration = None;
            self.ping = None;
            self.pending_applied = false;
        }
        if let Some(state) = effects.state {
            tracing::info!(operation="panel_state", identity=?self.identity, state=?state,
                "SugarCubes panel state changed");
        }
        let Some(message) = effects.send else { return Ok(()) };
        let configuration = matches!(message, PanelMessage::Configure { .. });
        let command = PanelCommand::new(
            self.identity,
            message,
            claim.ok_or("Panel command was not admitted")?,
        )?;
        if configuration {
            self.configuration = Some(command);
        } else {
            self.ping = Some(command);
        }
        Ok(())
    }

    /// Reject bounded overload without changing the handshake or failing the application.
    fn command_claim(&mut self) -> Option<MemoryClaim> {
        match self.lease.claim(PanelCommand::STORAGE_BYTES) {
            Ok(claim) => {
                self.admission_warned = false;
                Some(claim)
            }
            Err(cause) => {
                if !self.admission_warned {
                    tracing::warn!(operation="panel_command_admission", cause=%cause,
                        "Panel command admission is temporarily unavailable");
                    self.admission_warned = true;
                }
                None
            }
        }
    }

    /// Transfer one retained command while preserving its admitted allocation.
    pub(super) fn take_command(&mut self) -> Option<PanelCommand> {
        self.configuration.take().or_else(|| self.ping.take())
    }

    /// Transfer the next response while retaining its original ingress allocation.
    pub(super) fn take_canvas_reply(&mut self) -> Option<DesktopStartupCommand> {
        take_first(&mut self.canvas_replies).map(|reply| match reply {
            CanvasReply::Context(reply) => DesktopStartupCommand::SendCanvasContext(reply),
            CanvasReply::Surface(reply) => DesktopStartupCommand::SendSurface(reply),
        })
    }

    /// Correlate a surface request with the retained panel and its native admission.
    fn receive_surface(
        &mut self,
        identity: MainIdentity,
        message: CanvasSurfaceMessage,
        lease: &MemoryLease,
        claim: MemoryClaim,
    ) -> Result<()> {
        if self.identity != identity || !self.lease.same_reservation(lease) {
            return Err("Surface observation belongs to another native lifetime".into());
        }
        if self.policy.state() == PanelState::Failed {
            return Ok(());
        }
        if matches!(message, CanvasSurfaceMessage::Cancel { .. }) {
            return Ok(());
        }
        let Some(slot) = self.canvas_replies.iter_mut().find(|slot| slot.is_none()) else {
            tracing::warn!(operation="surface_reply_admission", identity=?identity,
                "Surface request rejected because the response queue is full");
            return Ok(());
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
                CanvasSurfaceDiscoveryStatus::Unsupported,
                &CanvasSurfaceCapabilities::default(),
                "",
                claim,
            )?,
            CanvasSurfaceMessage::Cancel { .. } => return Ok(()),
        };
        *slot = Some(CanvasReply::Surface(reply));
        Ok(())
    }

    /// Answer capability discovery for this exact panel without advertising an unowned surface.
    fn receive_context(
        &mut self,
        identity: MainIdentity,
        request: CanvasContextRequest,
        lease: &MemoryLease,
        claim: MemoryClaim,
    ) -> Result<()> {
        if self.identity != identity || !self.lease.same_reservation(lease) {
            return Err("Canvas context observation belongs to another native lifetime".into());
        }
        if self.policy.state() == PanelState::Failed {
            return Ok(());
        }
        let Some(slot) = self.canvas_replies.iter_mut().find(|slot| slot.is_none()) else {
            tracing::warn!(operation="canvas_context_reply_admission", identity=?identity,
                "Canvas context request rejected because the response queue is full");
            return Ok(());
        };
        *slot = Some(CanvasReply::Context(CanvasContextReply::new(
            identity,
            &request,
            CanvasDisplayCapability::Unavailable,
            claim,
        )?));
        Ok(())
    }

    /// Borrow the last checked application-clock deadline.
    pub(super) const fn deadline(&self) -> Option<Moment> {
        self.deadline
    }
}

impl ProductionChild {
    /// Route only an actual current, revealed main's panel observation into one policy owner.
    pub(super) fn observe_panel(
        &mut self,
        observation: PanelObservation,
        prepared: Option<&PreparedChildStartup>,
    ) -> Result<()> {
        let (identity, message, lease, _message_claim) = observation.into_parts();
        if self.shutdown.closing()
            || identity != self.identity
            || prepared.is_none_or(|prepared| prepared.session().phase() != StartupPhase::Running)
        {
            return Ok(());
        }
        if self.panel.is_none() && !matches!(message, PanelMessage::Ready(_)) {
            return Ok(());
        }
        let now = self.client()?.now()?;
        if self.panel.is_none() {
            let Some(panel) = PanelOwner::new(identity, lease.clone(), now)? else {
                if !self.panel_owner_admission_warned {
                    tracing::warn!(
                        operation = "panel_owner_admission",
                        "Panel session deferred because bounded storage is unavailable"
                    );
                    self.panel_owner_admission_warned = true;
                }
                return Ok(());
            };
            self.panel_owner_admission_warned = false;
            self.panel = Some(panel);
        }
        let panel = self.panel.as_mut().ok_or("Panel owner disappeared")?;
        if panel.identity != identity || !panel.lease.same_reservation(&lease) {
            return Err("Panel observation belongs to another native lifetime".into());
        }
        panel.receive(message, now)
    }

    /// Refuse surface operations until a native presentation owner has been admitted.
    pub(super) fn observe_surface(
        &mut self,
        observation: SurfaceObservation,
        prepared: Option<&PreparedChildStartup>,
    ) -> Result<()> {
        let (identity, message, lease, claim) = observation.into_parts();
        if self.shutdown.closing()
            || identity != self.identity
            || prepared.is_none_or(|prepared| prepared.session().phase() != StartupPhase::Running)
        {
            return Ok(());
        }
        let Some(panel) = self.panel.as_mut() else { return Ok(()) };
        panel.receive_surface(identity, message, &lease, claim)
    }

    /// Resolve context only through the currently revealed panel and its native allocation.
    pub(super) fn observe_canvas_context(
        &mut self,
        observation: CanvasContextObservation,
        prepared: Option<&PreparedChildStartup>,
    ) -> Result<()> {
        let (identity, request, lease, claim) = observation.into_parts();
        if self.shutdown.closing()
            || identity != self.identity
            || prepared.is_none_or(|prepared| prepared.session().phase() != StartupPhase::Running)
        {
            return Ok(());
        }
        let Some(panel) = self.panel.as_mut() else { return Ok(()) };
        panel.receive_context(identity, request, &lease, claim)
    }

    /// Advance the same session with the existing execution clock when its wake is due.
    pub(super) fn advance_panel(&mut self) -> Result<()> {
        if self.shutdown.closing() || self.panel.is_none() {
            return Ok(());
        }
        let now = self.client()?.now()?;
        self.panel.as_mut().ok_or("Panel owner disappeared")?.tick(now)
    }
}

#[cfg(test)]
#[path = "panel/tests.rs"]
mod tests;
