// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep the actual ordered restart effect until successful registered ingress is installed.
use super::super::restart::InstalledRestartIngress;
use super::*;
/// A pending same-source loan retains the installed projection and complete old receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production::entry_activation) enum RestartAcceptance {
    Pending,
    Accepted,
}
impl IncidentReporting {
    /// Capture only the actual ordered current Forward after real pipe and registry retirement.
    /// Occupied custody and every foreign head remain unchanged; no effect is reordered.
    pub(super) fn retain_current_restart(&mut self, host: &mut IncidentHost) -> Result<bool> {
        let Some(retired) = self.receiving_session.retired() else { return Ok(false) };
        let binding = retired.binding();
        if let Some(effect) = &self.retained_effect {
            return match effect {
                IncidentEffect::ForwardRestartIntent { run, acknowledgement } => {
                    validate_forward(binding, *run, *acknowledgement)?;
                    Ok(true)
                }
                _ => Ok(false),
            };
        }
        let Some(effect) = host.next_effect()? else { return Ok(true) };
        let IncidentEffect::ForwardRestartIntent { run, acknowledgement } = effect else {
            return Ok(true);
        };
        validate_forward(binding, *run, *acknowledgement)?;
        self.retained_effect = host.take_effect()?;
        Ok(true)
    }
    /// Issuance requires the actual accepted Domain retirement; ordered Forward custody stays owned.
    pub(in crate::production::entry_activation) fn restart_run(
        &mut self,
        host: &mut IncidentHost,
    ) -> Result<Option<RunKey>> {
        if !self.family_retired || self.store.is_some() || self.failure().is_some() {
            return Ok(None);
        }
        if !self.retain_current_restart(host)? {
            return Ok(None);
        }
        let Some(IncidentEffect::ForwardRestartIntent { run, acknowledgement }) =
            &self.retained_effect
        else {
            return Ok(None);
        };
        let binding =
            self.receiving_session.retired().ok_or("Actual Reporter retirement missing")?.binding();
        validate_forward(binding, *run, *acknowledgement)?;
        Ok(Some(*run))
    }
    /// Dispose only after the genuine successful registered projection is installed by its producer.
    /// Wrong source, run, generation or pending receipt preserves every original owner/effect.
    pub(in crate::production::entry_activation) fn restart_admitted(
        &mut self,
        ingress: &InstalledRestartIngress<'_>,
    ) -> Result<RestartAcceptance> {
        ingress.check()?;
        if !self.family_retired || self.failure().is_some() || self.store.is_some() {
            return Err("Actual accepted restart retirement missing".into());
        }
        let Some(IncidentEffect::ForwardRestartIntent { run, acknowledgement }) =
            &self.retained_effect
        else {
            return Err("Original forwarded restart effect missing".into());
        };
        self.receiving_session.validate_disposal()?;
        let Some(original) = self.receiving_session.recycled_mut() else {
            return Err("Actual same-source restart receipt missing".into());
        };
        validate_forward(original.binding, *run, *acknowledgement)?;
        let previous = ingress.previous();
        let next = ingress.run();
        if previous != *run
            || next.owner != previous.owner
            || next.instance != previous.instance
            || next.generation != previous.generation.next()?
            || !ingress.source().same_source(&original.startup)
            || !original.registered
            || original.receipt.is_none()
        {
            return Err("Actual installed registered restart source changed".into());
        }
        let relay = self.relay.as_mut().ok_or("Actual restart Reporter source missing")?;
        if !self.receiving_session.finish_disposal(relay, None)? {
            return Ok(RestartAcceptance::Pending);
        }
        self.retained_effect = None;
        self.reset_session_fields();
        Ok(RestartAcceptance::Accepted)
    }
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
