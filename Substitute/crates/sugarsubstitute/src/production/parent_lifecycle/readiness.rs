// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Join the authenticated desktop reveal with the retained main/browser/backend authorization.
use super::*;
use sugarsubstitute_application::supervision::MainAuthorizationObservation;
use sugarsubstitute_contracts::{lifecycle::RequestId, startup::MainIdentity};

impl ParentLifecycle {
    /// This signal establishes startup progress; it does not establish later process settlement.
    pub(super) fn main_revealed(
        &mut self,
        identity: MainIdentity,
        binding: ClientBinding,
        request: RequestId,
        resources: &mut ParentResources<'_, '_>,
    ) -> Result<()> {
        let accepted = self.apply_main_reveal(
            identity,
            binding,
            request,
            |owner| {
                resources.session.control.validate_current_admission()?;
                let pid =
                    owner.main.snapshot().browser_process_id.ok_or(ParentError::WrongPhase)?;
                if !owner
                    .environment
                    .as_ref()
                    .is_some_and(|proof| proof.validates(resources.profile, identity))
                    || !owner.browser.as_ref().is_some_and(|proof| {
                        proof.validates(resources.profile, identity, pid.get())
                    })
                {
                    return Err(ParentError::ForeignPeer.into());
                }
                Ok(())
            },
            || Ok(resources.execution.client.now()?),
        )?;
        if accepted {
            if matches!(
                resources.target,
                sugarsubstitute_contracts::lifecycle::BackendTarget::Managed(_)
            ) {
                resources.backend.pack_page_loaded(resources.execution)?;
            }
            resources.execution.authority.accept_installation(
                resources.execution.client,
                resources.finite,
                resources.execution.jobs,
            )?;
        } else {
            tracing::info!(operation="startup_readiness_deferred", identity=?identity,
                "Main reveal raced with retirement or backend loss; startup remains unconfirmed");
        }
        Ok(())
    }

    /// Keep native admission/proof queries at the adapter boundary and ordering in this owner.
    pub(in crate::production::parent_lifecycle) fn apply_main_reveal(
        &mut self,
        identity: MainIdentity,
        binding: ClientBinding,
        request: RequestId,
        validate: impl FnOnce(&Self) -> Result<()>,
        now: impl FnOnce() -> Result<Moment>,
    ) -> Result<bool> {
        if self.closed
            || self.cleanup.is_some()
            || self.failure.is_some()
            || self.main_retirement.is_some()
            || self.attempt_cleanup.is_some()
            || self.recovery.live_lease() != self.lease
        {
            return Ok(false);
        }
        if self.profile_pending.is_some()
            || self.main.snapshot().lease != self.lease
            || self.lease.is_none()
        {
            return Err(ParentError::WrongPhase.into());
        }
        validate(self)?;
        self.main.accept(
            binding,
            request,
            MainAuthorizationObservation::MainRevealed { identity },
            now()?,
        )?;
        tracing::info!(operation="startup_readiness_observed", identity=?identity,
            "Authenticated desktop reported its ready frontend and revealed main window");
        Ok(true)
    }
}
