// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Hold the actual authenticated Restart request until genuine fresh-family admission exists.
use super::*;
use sugarsubstitute_execution::RegisteredRecoveryGrant;
use sugarsubstitute_supervision::{WindowsProcessRegistry, profiles::WindowsProfileLease};
/// Passive selection of the exact still-owned received request; this grants no process authority.
#[derive(Clone, Copy)]
pub(in crate::production::entry_activation) struct DeferredFreshRestart {
    command: ReportCommand,
    context: IncidentRequestContext,
    relay_until: Moment,
}
impl DeferredFreshRestart {
    pub(super) const fn new(
        command: ReportCommand,
        context: IncidentRequestContext,
        relay_until: Moment,
    ) -> Self {
        Self { command, context, relay_until }
    }
    pub(in crate::production::entry_activation) const fn binding(self) -> ReportBinding {
        self.command.binding()
    }
    pub(in crate::production::entry_activation) const fn context(self) -> IncidentRequestContext {
        self.context
    }
    pub(in crate::production::entry_activation) const fn original_until(self) -> Moment {
        self.context.budget().until()
    }
}
impl IncidentReporting {
    /// Release only inert deferred selection after the existing checked Reporter close owner
    /// returned its original frame and retired the family, pipe and files. This is no ACK.
    pub(in crate::production::entry_activation) fn dispose_deferred_fresh_restart_after_retirement(
        &mut self,
        expected: ReportBinding,
    ) -> Result<bool> {
        if !self.settled() {
            return Ok(false);
        }
        if self.request.is_some() || self.completion.is_some() {
            return Err(
                "Actual deferred Restart frame remains owned after reported retirement".into()
            );
        }
        if self.deferred_restart.is_some_and(|original| original.binding() != expected) {
            return Err("Actual deferred Restart retirement binding changed".into());
        }
        self.deferred_restart = None;
        Ok(true)
    }

    /// Borrow the selected request while its actual frame/token remain retained here.
    pub(in crate::production::entry_activation) fn deferred_fresh_restart(
        &self,
    ) -> Option<DeferredFreshRestart> {
        self.deferred_restart
    }
    /// Match this incident to the actual current registered run, including recovered old incidents.
    pub(in crate::production::entry_activation) fn fresh_restart_previous(
        &self,
        owner: sugarsubstitute_contracts::lifecycle::CoordinatorKey,
    ) -> Result<RunKey> {
        let selected = self.deferred_restart.ok_or("Actual deferred Restart missing")?;
        if self.binding != Some(selected.binding()) || self.request.is_none() {
            return Err("Actual deferred request custody changed".into());
        }
        if let IncidentSubject::Desktop(run) = selected.binding().incident().subject()
            && run.owner == owner
        {
            return Ok(run);
        }
        let current =
            self.recovery_source.as_ref().ok_or("Actual current recovery startup missing")?;
        if current.run.owner != owner
            || selected.binding().incident().subject().coordinator().namespace != owner.namespace
        {
            return Err("Actual recovery Root changed".into());
        }
        Ok(current.run)
    }
    /// Permit actual durable ACK only after the same source owns held creation/profile/watch inputs.
    /// Original received capability and policy context stay here through the repository CAS.
    pub(in crate::production::entry_activation) fn acknowledge_prepared_fresh_restart(
        &mut self,
        host: &mut IncidentHost,
        grant: &RegisteredRecoveryGrant,
        profile: &WindowsProfileLease,
        previous: &WindowsProfileLease,
        registry: &WindowsProcessRegistry,
    ) -> Result<bool> {
        let selected = self.deferred_restart.ok_or("Actual deferred Restart missing")?;
        if selected.binding() != grant.key().report()
            || self.binding != Some(selected.binding())
            || self.request.as_ref().is_none_or(|request| request.context != selected.context)
        {
            return Err("Actual prepared Restart source changed".into());
        }
        if !profile.recovery_launch_acknowledgement_allowed(registry, grant, previous)? {
            return Ok(false);
        }
        grant.check()?;
        self.enqueue_prepared_restart_acknowledgement(host, selected.command, selected.context)?;
        self.original = Some(selected.context);
        self.dismiss_action = Some(ReportAction::DismissAndRequestRestart);
        self.close_request =
            self.request.as_ref().map(|request| request.authentication.issued.request());
        self.deferred_restart = None;
        Ok(true)
    }
    /// Retain the genuine policy effect and one native turn after fresh preparation completed.
    /// This grants no native frame, write capability or durable acknowledgement.
    pub(super) fn enqueue_prepared_restart_acknowledgement(
        &mut self,
        host: &mut IncidentHost,
        command: ReportCommand,
        context: IncidentRequestContext,
    ) -> Result<()> {
        if host.report_action(command, context)?.is_some() {
            return Err("Unexpected copy authorization for Restart".into());
        }
        self.continuation = Some(ReportContinuation::ConsumeEffect);
        Ok(())
    }
    /// Return actionable rejection without ACK when genuine preparation cannot complete.
    pub(in crate::production::entry_activation) fn reject_deferred_fresh_restart(
        &mut self,
    ) -> Result<()> {
        let selected = self.deferred_restart.ok_or("Actual deferred Restart missing")?;
        self.reject_restart(selected.binding(), selected.relay_until)?;
        self.complete_observation()?;
        self.deferred_restart = None;
        Ok(())
    }
    /// Borrow only the genuine incident policy's durable forward after Reporter retirement.
    pub(in crate::production::entry_activation) fn fresh_restart_acknowledgement(
        &mut self,
        host: &mut IncidentHost,
        grant: &RegisteredRecoveryGrant,
    ) -> Result<Option<DurableAcknowledgement>> {
        if !self.family_retired || self.store.is_some() || self.failure().is_some() {
            return Ok(None);
        }
        if !self.retain_current_restart(host)? {
            return Ok(None);
        }
        let binding =
            self.receiving_session.retired().ok_or("Actual Reporter retirement missing")?.binding();
        if binding != grant.key().report() {
            return Err("Actual fresh Restart binding changed".into());
        }
        match self.retained_effect.as_ref() {
            Some(IncidentEffect::ForwardRestartIntent { run, acknowledgement })
                if *run == grant.key().previous() =>
            {
                Ok(Some(*acknowledgement))
            }
            Some(IncidentEffect::ForwardRecoveredRestartIntent { incident, acknowledgement })
                if *incident == binding.incident() =>
            {
                Ok(Some(*acknowledgement))
            }
            None => Ok(None),
            _ => Err("Actual fresh durable Restart changed".into()),
        }
    }
    /// Dispose the original same-source Reporter only after genuine fresh creation is committed.
    pub(in crate::production::entry_activation) fn accept_fresh_restart(
        &mut self,
        grant: &RegisteredRecoveryGrant,
    ) -> Result<bool> {
        grant.check()?;
        let binding = self.receiving_session.validate_disposal()?;
        let original = self
            .receiving_session
            .recycled()
            .ok_or("Actual same-source retirement receipt missing")?;
        if binding != grant.key().report()
            || !original.registered
            || original.receipt.is_none()
            || !grant.same_startup_source(&original.startup)
        {
            return Err("Actual fresh grant or retired Reporter source changed".into());
        }
        let relay = self.relay.as_mut().ok_or("Actual fresh Restart relay missing")?;
        if !self.receiving_session.finish_disposal(relay, None)? {
            return Ok(false);
        }
        self.retained_effect = None;
        self.reset_session_fields();
        Ok(true)
    }
}
