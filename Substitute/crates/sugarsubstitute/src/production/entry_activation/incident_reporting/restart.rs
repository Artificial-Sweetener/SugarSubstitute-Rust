// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Route actual durable report intents through current registered source and checked retirement.
use super::super::restart::InstalledRestartIngress;
use super::*;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production::entry_activation) enum RestartAcceptance {
    Pending,
    Accepted,
}
/// Only CurrentRecovery requests close of a current live run; Existing keeps the original live flow.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production::entry_activation) enum RestartTarget {
    Existing(RunKey),
    CurrentRecovery(RunKey),
}
impl RestartTarget {
    pub(in crate::production::entry_activation) fn run(self) -> RunKey {
        match self {
            Self::Existing(run) | Self::CurrentRecovery(run) => run,
        }
    }
}
impl IncidentReporting {
    fn target(
        &self,
        binding: ReportBinding,
        effect: &IncidentEffect,
    ) -> Result<Option<RestartTarget>> {
        match effect {
            IncidentEffect::ForwardRestartIntent { run, acknowledgement } => {
                validate_forward(binding, *run, *acknowledgement)?;
                Ok(Some(RestartTarget::Existing(*run)))
            }
            IncidentEffect::ForwardRecoveredRestartIntent { incident, acknowledgement } => {
                validate_recovered_forward(binding, *incident, *acknowledgement)?;
                let current = self
                    .recovery_source
                    .as_ref()
                    .ok_or("Actual current recovery startup missing")?;
                let key = self.key.ok_or("Actual current Reporter key missing")?;
                if current.run.owner != key.owner()
                    || incident.subject().coordinator().namespace != key.owner().namespace
                {
                    return Err("Actual recovered restart current owner changed".into());
                }
                Ok(Some(RestartTarget::CurrentRecovery(current.run)))
            }
            _ => Ok(None),
        }
    }
    pub(super) fn retain_current_restart(&mut self, host: &mut IncidentHost) -> Result<bool> {
        let Some(retired) = self.receiving_session.retired() else {
            return Ok(false);
        };
        let binding = retired.binding();
        if let Some(effect) = &self.retained_effect {
            return Ok(self.target(binding, effect)?.is_some());
        }
        let Some(effect) = host.next_effect()? else {
            return Ok(true);
        };
        if self.target(binding, effect)?.is_some() {
            self.retained_effect = host.take_effect()?;
        }
        Ok(true)
    }
    /// Intent alone owns no process. The caller still closes and receives the actual selected run.
    pub(in crate::production::entry_activation) fn restart_run(
        &mut self,
        host: &mut IncidentHost,
    ) -> Result<Option<RestartTarget>> {
        if !self.family_retired || self.store.is_some() || self.failure().is_some() {
            return Ok(None);
        }
        if !self.retain_current_restart(host)? {
            return Ok(None);
        }
        let Some(effect) = &self.retained_effect else {
            return Ok(None);
        };
        let binding =
            self.receiving_session.retired().ok_or("Actual Reporter retirement missing")?.binding();
        self.target(binding, effect)
    }
    pub(in crate::production::entry_activation) fn restart_admitted(
        &mut self,
        ingress: &InstalledRestartIngress<'_>,
    ) -> Result<RestartAcceptance> {
        ingress.check()?;
        if !self.family_retired || self.failure().is_some() || self.store.is_some() {
            return Err("Actual accepted restart retirement missing".into());
        }
        let binding = self.receiving_session.validate_disposal()?;
        let effect = self.retained_effect.as_ref().ok_or("Original forwarded restart missing")?;
        let target = self.target(binding, effect)?.ok_or("Actual report restart intent absent")?;
        let original = self
            .receiving_session
            .recycled()
            .ok_or("Actual same-source retirement receipt missing")?;
        let previous = ingress.previous();
        let next = ingress.run();
        if previous != target.run()
            || next.owner != previous.owner
            || next.instance != previous.instance
            || next.generation != previous.generation.next()?
            || !ingress.source().same_source(&original.startup)
            || !original.registered
            || original.receipt.is_none()
        {
            return Err("Actual installed current-source restart changed".into());
        }
        if let RestartTarget::CurrentRecovery(_) = target {
            let current =
                self.recovery_source.as_ref().ok_or("Actual current recovery startup missing")?;
            if !current.startup.same_source(ingress.source()) {
                return Err("Fresh recovery startup source changed".into());
            }
        }
        let relay = self.relay.as_mut().ok_or("Actual restart Reporter source missing")?;
        if !self.receiving_session.finish_disposal(relay, None)? {
            return Ok(RestartAcceptance::Pending);
        }
        if let Some(current) = &mut self.recovery_source
            && current.run == previous
            && current.startup.same_source(ingress.source())
        {
            current.run = next;
        }
        self.retained_effect = None;
        self.reset_session_fields();
        Ok(RestartAcceptance::Accepted)
    }
}
fn validate_recovered_forward(
    binding: ReportBinding,
    incident: IncidentKey,
    acknowledgement: DurableAcknowledgement,
) -> Result<()> {
    if binding.incident() != incident
        || acknowledgement.incident() != incident
        || binding.revision().next()? != acknowledgement.revision()
    {
        return Err("Actual recovered report durable acknowledgement changed".into());
    }
    Ok(())
}
fn validate_forward(
    binding: ReportBinding,
    run: RunKey,
    acknowledgement: DurableAcknowledgement,
) -> Result<()> {
    if binding.incident() != acknowledgement.incident()
        || binding.revision().next()? != acknowledgement.revision()
        || acknowledgement.incident().subject() != IncidentSubject::Desktop(run)
    {
        return Err("Actual retired durable restart correlation changed".into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "restart/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "restart/recovered_tests.rs"]
mod recovered_tests;
