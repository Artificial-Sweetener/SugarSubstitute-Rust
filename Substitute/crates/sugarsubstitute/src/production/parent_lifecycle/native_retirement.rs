// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Bind actual native Main absence/retirement before acknowledging attempt cleanup.
use super::{ParentError, ParentLifecycle, ParentResources, Reply, Result};
use sugarsubstitute_application::supervision::{
    MainAuthorizationObservation, MainAuthorizationStage,
};
use sugarsubstitute_contracts::{lifecycle::ClientBinding, startup::MainIdentity};
use sugarsubstitute_supervision::lifecycle_protocol::WireHeader;

impl ParentLifecycle {
    /// Validate a genuine creating-thread observation against the retained current
    /// acquisition. Parent's empty layout supplies no native absence observation.
    pub(super) fn observe_native_retirement(
        &mut self,
        binding: ClientBinding,
        identity: MainIdentity,
        header: WireHeader,
        native_profile_pending: bool,
    ) -> Result<bool> {
        if self.main_retirement.is_some() {
            return Err(ParentError::WrongPhase.into());
        }
        let negative = self.main.stage() == MainAuthorizationStage::Retired;
        if negative {
            if native_profile_pending
                || self.profile_pending.is_some()
                || self.environment.is_some()
                || self.browser.is_some()
                || self.main.snapshot().identity.is_some_and(|old| {
                    old.run != identity.run || old.launch.get() >= identity.launch.get()
                })
            {
                return Err(ParentError::WrongPhase.into());
            }
        } else if self.main.snapshot().identity != Some(identity) {
            return Err(ParentError::ForeignPeer.into());
        }
        self.acquisition
            .as_mut()
            .ok_or(ParentError::StaleRequest)?
            .observe_retirement(binding, identity, header)?;
        if let Some(attempt) = &mut self.attempt_cleanup {
            attempt.observe_retirement(identity, header)?;
        }
        Ok(negative)
    }

    /// The authenticated Child sends this operation only after its actual MainFence
    /// absence or real actor retirement. Profile work retains its original proof path.
    pub(super) fn native_main_retired(
        &mut self,
        index: usize,
        identity: MainIdentity,
        resources: &mut ParentResources<'_, '_>,
    ) -> Result<()> {
        let header = self.requests.get(index)?.header;
        let binding = self.request_binding(header)?;
        if self.observe_native_retirement(
            binding,
            identity,
            header,
            resources.profile.snapshot().pending,
        )? {
            if let Some(refresh) = &mut self.frontend_refresh {
                refresh.close(sugarsubstitute_contracts::lifecycle::ShutdownBudget::new(
                    resources.execution.client.now()?,
                )?);
                refresh.absent_ack = Some(index);
                refresh.schedule_cleanup(resources)?;
                if !refresh.retired() {
                    return Ok(());
                }
            }
            self.frontend_refresh = None;
            self.requests.get_mut(index)?.reply = Some(Reply::Acknowledged);
            self.trace_retirement_ack(header, super::retirement_trace::AckStage::Staged, None);
            return Ok(());
        }
        self.main.retire(identity)?;
        resources.profile.retire_main_attempt(identity)?;
        self.main.accept(
            binding,
            header.request.ok_or(ParentError::WrongPhase)?,
            MainAuthorizationObservation::MainRetired { identity },
            resources.execution.client.now()?,
        )?;
        self.environment = None;
        self.browser = None;
        self.main_retirement = Some((index, identity));
        tracing::info!(operation="parent_main_retirement_accepted",run=?self.run,
            request=?header.request,identity=?identity,
            profile_pending=self.profile_pending.is_some(),native_profile_pending=resources.profile.snapshot().pending,
            "Original MainRetired observation accepted before its acknowledgement");
        self.finish_main_retirement(resources)
    }
}
