// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Correlate real repository activation proofs before projecting pure incident observations.
use super::*;
impl IncidentHost {
    pub(super) fn vacant(&mut self) -> Result<&mut PolicyState, HostIssue> {
        let state = self.state()?;
        if state.effects.iter().any(Option::is_some) {
            return Err(HostIssue::Pending);
        }
        Ok(state)
    }
    pub(super) fn retain_effects(state: &mut PolicyState, effects: IncidentEffects) {
        for incident in state.coordinator.report_admission_failures() {
            if let Some(cause) = state.coordinator.report_admission_failure(incident) {
                tracing::warn!(
                    operation = "deferred_report_admission",
                    incident = ?incident,
                    cause = ?cause,
                    "Original deferred report refusal remains retained"
                );
            }
        }
        for (slot, effect) in state.effects.iter_mut().zip(effects) {
            *slot = Some(effect);
        }
    }
    /// Admit only the SAME resource policy's already enrolled full run/profile identity.
    pub(in crate::production) fn seed(
        &mut self,
        resources: &mut ResourceCoordinator,
        seed: IncidentSeed,
        context: IncidentRequestContext,
    ) -> Result<(), HostIssue> {
        let state = self.vacant()?;
        match state.coordinator.seed(resources, seed, context) {
            Ok(effects) => {
                Self::retain_effects(state, effects);
                Ok(())
            }
            Err(cause) => {
                self.cause.get_or_insert(HostCause::Decision(cause));
                Err(HostIssue::Decision)
            }
        }
    }
    /// Admit the unchanged sealed actual failed exit; native status is never reclassified.
    pub(in crate::production) fn failed_desktop(
        &mut self,
        resources: &mut ResourceCoordinator,
        fact: FailedDesktopFact,
        failure: FailureEvidence,
        context: IncidentRequestContext,
    ) -> Result<(), HostIssue> {
        let state = self.vacant()?;
        match state.coordinator.failed_desktop(resources, fact, failure, context) {
            Ok(effects) => {
                Self::retain_effects(state, effects);
                Ok(())
            }
            Err(cause) => {
                self.cause.get_or_insert(HostCause::Decision(cause));
                Err(HostIssue::Decision)
            }
        }
    }
    /// Borrow the next actual policy decision; rejection by its adapter leaves it intact.
    pub(in crate::production) fn next_effect(
        &mut self,
    ) -> Result<Option<&IncidentEffect>, HostIssue> {
        Ok(self.state()?.effects.iter().find_map(Option::as_ref))
    }
    /// Transfer exactly one ordered effect to its retained genuine adapter admission.
    pub(in crate::production) fn take_effect(
        &mut self,
    ) -> Result<Option<IncidentEffect>, HostIssue> {
        Ok(self.state()?.effects.iter_mut().find(|slot| slot.is_some()).and_then(Option::take))
    }
    pub(super) fn observe(
        &mut self,
        observation: IncidentServiceObservation,
    ) -> Result<(), HostIssue> {
        let state = self.vacant()?;
        match state.coordinator.observe(observation) {
            Ok(effects) => {
                Self::retain_effects(state, effects);
                Ok(())
            }
            Err(cause) => {
                self.cause.get_or_insert(HostCause::Decision(cause));
                Err(HostIssue::Decision)
            }
        }
    }
    /// Consume only the service's retained actual seed proof after checked file cleanup.
    /// The parent cannot launch from worker success alone or from a copied seed record.
    pub(in crate::production) fn adopt_seed_service(
        &mut self,
        scope: IncidentOperationScope,
    ) -> Result<(), HostIssue> {
        if self.closing {
            return Err(HostIssue::Closed);
        }
        let controller = match self.completion.as_mut().map(|value| &mut value.outcome) {
            Some(JobOutcome::Success(controller)) => controller,
            _ => return Err(HostIssue::Unprepared),
        };
        let policy = &mut controller.policy;
        let state = policy.state.as_mut().ok_or(HostIssue::Pending)?;
        if state.state.effects.iter().any(Option::is_some)
            || !policy.seed.files_retired()
            || policy.seed.outcome() != Some(TerminalKind::Success)
        {
            return Err(HostIssue::Pending);
        }
        let observation = policy
            .seed
            .inspect(|operation| {
                let proof = operation.committed().ok_or(HostIssue::Preparation)?;
                if operation.failure().is_some()
                    || operation.memory_failure().is_some()
                    || proof.operation() != scope.operation()
                    || proof.key() != scope.incident()
                    || proof.seed().revision() != scope.expected_revision()
                {
                    return Err(HostIssue::Identity);
                }
                Ok(IncidentServiceObservation::SeedCommitted { scope, seed: proof.seed().clone() })
            })
            .ok_or(HostIssue::Pending)??;
        match state.state.coordinator.observe(observation) {
            Ok(effects) => {
                Self::retain_effects(&mut state.state, effects);
                policy.seed.release_adopted().map_err(|_| HostIssue::Pending)?;
                Ok(())
            }
            Err(cause) => {
                self.cause.get_or_insert(HostCause::Decision(cause));
                Err(HostIssue::Decision)
            }
        }
    }
}
