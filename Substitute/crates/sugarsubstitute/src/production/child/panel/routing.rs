// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Consume current native ingress before returning each lane's original delivery credit.

use super::*;
use sugarsubstitute_application::startup::StartupPhase;
use sugarsubstitute_contracts::{
    canvas_native::CanvasNativeRequest, canvas_surface::CanvasSurfaceMessage,
};
use sugarsubstitute_desktop::{
    CanvasNativeObservation, PanelIngressReceipt, PanelObservation, SurfaceObservation,
};

#[cfg(test)]
#[path = "routing/tests.rs"]
mod tests;

impl ProductionChild {
    /// Route current controls synchronously and retain their original claim until consumption delivery.
    pub(in crate::production::child) fn observe_panel(
        &mut self,
        observation: PanelObservation,
        prepared: Option<&PreparedChildStartup>,
    ) -> Result<()> {
        let (identity, message, lease, claim) = observation.into_parts();
        let running =
            prepared.is_some_and(|prepared| prepared.session().phase() == StartupPhase::Running);
        self.receive_panel_control(identity, message, lease, claim, running)
    }

    /// Couple synchronous policy processing to its consumption receipt, including refusals.
    fn receive_panel_control(
        &mut self,
        identity: MainIdentity,
        message: PanelMessage,
        lease: MemoryLease,
        claim: MemoryClaim,
        running: bool,
    ) -> Result<()> {
        if !self.accepts_panel_ingress(identity) {
            return Ok(());
        }
        self.bind_panel_delivery(identity, &lease)?;
        self.panel_deliveries.check_control()?;
        let receipt = PanelIngressReceipt::control(identity, claim)?;
        let consumed = self.consume_panel(identity, message, lease, running);
        self.panel_deliveries.complete_control(receipt)?;
        consumed
    }

    /// Keep policy admission and retry behavior independent of ingress acknowledgement delivery.
    fn consume_panel(
        &mut self,
        identity: MainIdentity,
        message: PanelMessage,
        lease: MemoryLease,
        running: bool,
    ) -> Result<()> {
        if self.shutdown.closing()
            || !running
            || (self.panel.is_none() && !matches!(message, PanelMessage::Ready(_)))
        {
            return Ok(());
        }
        let now = self.client()?.now()?;
        if self.panel.is_none() {
            let Some(panel) = PanelOwner::new(identity, lease, now, self.system_accent)? else {
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
        self.panel.as_mut().ok_or("Panel owner disappeared")?.receive(message, now)
    }

    /// Refuse current surface requests without depending on panel readiness or policy success.
    pub(in crate::production::child) fn observe_surface(
        &mut self,
        observation: SurfaceObservation,
        _prepared: Option<&PreparedChildStartup>,
    ) -> Result<()> {
        let (identity, message, lease, claim) = observation.into_parts();
        self.receive_surface_message(identity, message, lease, claim)
    }

    /// Consume the attributed surface data after the native observation transfers its custody.
    fn receive_surface_message(
        &mut self,
        identity: MainIdentity,
        message: CanvasSurfaceMessage,
        lease: MemoryLease,
        claim: MemoryClaim,
    ) -> Result<()> {
        if !self.accepts_panel_ingress(identity) {
            return Ok(());
        }
        self.bind_panel_delivery(identity, &lease)?;
        self.panel_deliveries.surface(identity, message, &lease, claim)
    }

    /// Return an explicit unavailable answer through the same current native delivery owner.
    pub(in crate::production::child) fn observe_canvas_native(
        &mut self,
        observation: CanvasNativeObservation,
        _prepared: Option<&PreparedChildStartup>,
    ) -> Result<()> {
        // The refusal is encoded at once, so upload storage is released with the request.
        let (identity, request, lease, claim, _storage) = observation.into_parts();
        self.receive_native_request(identity, &request, lease, claim)
    }

    /// Admit a canvas request only for the current navigation, its storage and a configured panel.
    ///
    /// # Errors
    /// Names the first fact that fails, so a refusal can tell the page why.
    #[cfg(feature = "native-composition")]
    pub(in crate::production::child) fn admission(
        &self,
        identity: MainIdentity,
        lease: &MemoryLease,
    ) -> std::result::Result<(), crate::startup::CanvasRefusal> {
        use crate::startup::CanvasRefusal;
        if self.shutdown.closing() {
            return Err(CanvasRefusal::Closing);
        }
        if self.identity != identity {
            return Err(CanvasRefusal::OtherNavigation);
        }
        self.panel.as_ref().ok_or(CanvasRefusal::PanelNotConfigured)?.admission(identity, lease)
    }

    /// Consume an attributed native request without replacing its original response admission.
    fn receive_native_request(
        &mut self,
        identity: MainIdentity,
        request: &CanvasNativeRequest,
        lease: MemoryLease,
        claim: MemoryClaim,
    ) -> Result<()> {
        if !self.accepts_panel_ingress(identity) {
            return Ok(());
        }
        self.bind_panel_delivery(identity, &lease)?;
        // The application routed this request here: either the page does not hold canvas
        // authority, or the application is not running it for a composition-hosted page.
        #[cfg(feature = "native-composition")]
        let unavailable = self
            .admission(identity, &lease)
            .err()
            .unwrap_or(crate::startup::CanvasRefusal::NativeCanvasNotGranted)
            .failure();
        #[cfg(not(feature = "native-composition"))]
        let unavailable = sugarsubstitute_contracts::canvas_native::CanvasFailure::new(
            sugarsubstitute_contracts::canvas_native::CanvasFailureStatus::Unavailable,
        );
        self.panel_deliveries.native(identity, request, &lease, claim, &unavailable)
    }

    /// A retired or foreign peer cannot bind delivery resources for the current page.
    fn accepts_panel_ingress(&self, identity: MainIdentity) -> bool {
        identity == self.identity && !self.shutdown.main_retired && !self.shutdown.native_retired()
    }

    /// Reuse current native provenance even when its policy has failed or is not yet constructed.
    fn bind_panel_delivery(&mut self, identity: MainIdentity, lease: &MemoryLease) -> Result<()> {
        if self.panel.as_ref().is_some_and(|panel| !panel.same_lifetime(identity, lease)) {
            return Err("Panel observation belongs to another native lifetime".into());
        }
        self.panel_deliveries.bind(identity, lease)
    }

    /// Retain the operating-system accent for future pages and reconfigure the live page.
    pub(in crate::production::child) fn adopt_system_accent(
        &mut self,
        accent: Option<AccentColor>,
    ) -> Result<()> {
        self.system_accent = accent;
        if self.shutdown.closing() {
            return Ok(());
        }
        let now = match &self.panel {
            Some(_) => self.client()?.now()?,
            None => return Ok(()),
        };
        self.panel.as_mut().ok_or("Panel owner disappeared")?.update_accent(accent, now)
    }

    /// Advance the same session with the existing execution clock when its wake is due.
    pub(in crate::production::child) fn advance_panel(&mut self) -> Result<()> {
        if self.shutdown.closing() || self.panel.is_none() {
            return Ok(());
        }
        let now = self.client()?.now()?;
        self.panel.as_mut().ok_or("Panel owner disappeared")?.tick(now)
    }
}
