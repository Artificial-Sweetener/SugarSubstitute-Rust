// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose panel handshake decisions with the current native document and its memory admission.

use super::*;
use std::time::Duration;
use sugarsubstitute_application::panel::{PanelEffects, PanelHostSession, PanelState};
use sugarsubstitute_contracts::{
    appearance::AccentColor,
    host::{HostCapability, HostDescription, HostProtocol},
    panel::{PanelMessage, PanelPalette, PanelPresentation, PanelSession, PanelStyle},
};
use sugarsubstitute_desktop::{MainHosting, PanelCommand};

#[path = "panel/deliveries.rs"]
mod deliveries;
#[path = "panel/routing.rs"]
mod routing;
pub(super) use deliveries::Deliveries;

const OWNER_BYTES: u64 = 4096;
const ADMISSION_RETRY: Duration = Duration::from_millis(100);

/// Retain one current page's policy and at most one pending message of each host kind.
pub(super) struct PanelOwner {
    identity: MainIdentity,
    policy: PanelHostSession,
    lease: MemoryLease,
    configuration: Option<PanelCommand>,
    ping: Option<PanelCommand>,
    pending_applied: bool,
    /// A changed presentation awaiting command admission before the policy adopts it.
    pending_presentation: Option<PanelPresentation>,
    deadline: Option<Moment>,
    admission_warned: bool,
    _claim: MemoryClaim,
}

impl PanelOwner {
    /// Check the policy owner's original navigation and reservation independently of its state.
    fn same_lifetime(&self, identity: MainIdentity, lease: &MemoryLease) -> bool {
        self.identity == identity && self.lease.same_reservation(lease)
    }

    /// Check the current navigation and its original admitted native allocation.
    ///
    /// # Errors
    /// Names whether the navigation, the allocation or the handshake outcome fails.
    #[cfg(feature = "native-composition")]
    pub(super) fn admission(
        &self,
        identity: MainIdentity,
        lease: &MemoryLease,
    ) -> std::result::Result<(), crate::startup::CanvasRefusal> {
        use crate::startup::CanvasRefusal;
        if self.identity != identity {
            return Err(CanvasRefusal::OtherNavigation);
        }
        if !self.lease.same_reservation(lease) {
            return Err(CanvasRefusal::OtherLifetime);
        }
        if self.policy.state() == PanelState::Failed {
            return Err(CanvasRefusal::PanelFailed);
        }
        Ok(())
    }

    /// Claim the persistent state before constructing the navigation-bound panel policy.
    /// The page is configured with the current operating-system `accent`, when reported, and
    /// told what the host serves to a browser in `hosting`.
    fn new(
        identity: MainIdentity,
        lease: MemoryLease,
        now: Moment,
        accent: Option<AccentColor>,
        hosting: MainHosting,
    ) -> Result<Option<Self>> {
        let claim = match lease.claim(OWNER_BYTES) {
            Ok(claim) => claim,
            Err(_) => return Ok(None),
        };
        let session = PanelSession::for_main(identity)?;
        let policy =
            PanelHostSession::new(session, description(hosting)?, presentation(accent)?, now)?;
        let deadline = policy.next_deadline()?;
        Ok(Some(Self {
            identity,
            policy,
            lease,
            configuration: None,
            ping: None,
            pending_applied: false,
            pending_presentation: None,
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

    /// Reconfigure the live page with a changed operating-system accent.
    /// Without command capacity the newest presentation waits for a bounded retry.
    fn update_accent(&mut self, accent: Option<AccentColor>, now: Moment) -> Result<()> {
        self.pending_presentation = Some(presentation(accent)?);
        self.adopt_pending_presentation()?;
        if self.pending_presentation.is_some() {
            self.schedule_pending(now)?;
        }
        Ok(())
    }

    /// Hand the waiting presentation to the policy only once its command can be admitted.
    fn adopt_pending_presentation(&mut self) -> Result<()> {
        let Some(presentation) = self.pending_presentation.take() else { return Ok(()) };
        if self.policy.state() == PanelState::Failed {
            return Ok(());
        }
        let Some(claim) = self.command_claim() else {
            self.pending_presentation = Some(presentation);
            return Ok(());
        };
        let effects = self.policy.update_presentation(presentation);
        self.adopt(effects, Some(claim))
    }

    /// Advance only an actual due deadline without manufacturing a timer or task.
    fn tick(&mut self, now: Moment) -> Result<()> {
        if self.deadline.is_none_or(|deadline| now < deadline) {
            return Ok(());
        }
        self.adopt_pending_presentation()?;
        self.tick_policy(now)?;
        if self.pending_presentation.is_some() {
            self.schedule_pending(now)?;
        }
        Ok(())
    }

    /// Advance the handshake and heartbeat policy at a due deadline.
    fn tick_policy(&mut self, now: Moment) -> Result<()> {
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
        if let PanelMessage::Configure { presentation, .. } = &message {
            tracing::info!(operation="panel_accent_configure", identity=?self.identity,
                accent=?presentation.accent_color(), "Configuring the frontend accent");
        }
        let configuration = matches!(message, PanelMessage::Configure { .. });
        let command = PanelCommand::new(
            self.identity,
            effects.description,
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

    /// Borrow the last checked application-clock deadline.
    pub(super) const fn deadline(&self) -> Option<Moment> {
        self.deadline
    }
}

/// What this host serves and declares to a page in a browser with `hosting`.
///
/// The page keeps its saved appearance, so presentation styles are not declared. Only a
/// composition-hosted browser presents output on the native canvas.
fn description(hosting: MainHosting) -> Result<HostDescription> {
    let mut capabilities = Vec::new();
    if hosting.presents_native_canvas() {
        capabilities.push(HostCapability::HOST_OUTPUT_SURFACE);
    }
    Ok(HostDescription::new(
        vec![HostProtocol::PANEL, HostProtocol::CANVAS, HostProtocol::MEMORY],
        capabilities,
    )?)
}

/// Host presentation for the SugarCubes page; the accent is the operating-system accent.
fn presentation(accent: Option<AccentColor>) -> Result<PanelPresentation> {
    Ok(PanelPresentation::new(
        false,
        PanelStyle::Comfy,
        Some(PanelPalette::DarkGray),
        accent.map(AccentColor::css_hex),
        0,
    )?)
}

#[cfg(test)]
#[path = "panel/tests.rs"]
mod tests;
