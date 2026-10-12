// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Require independently observed seed, source, family and reporter settlement.
use super::*;
use crate::supervision::DesktopExitTransaction;
use sugarsubstitute_contracts::{incident::*, lifecycle::RunKey};

impl IncidentCoordinator {
    /// Retire a clean seed only from the actual accepted and settled exit owner.
    /// Neither a raw exit nor a requested completion can remove a retained seed.
    pub fn completed_desktop(
        &mut self,
        exit: &DesktopExitTransaction,
    ) -> Result<IncidentEffects, IncidentDecisionError> {
        let (run, _process) = exit.completed_incident_seed()?;
        let key = IncidentKey::new(IncidentSubject::Desktop(run));
        let index = self.index(key)?;
        let record = self.record(index)?;
        if record.failure.is_some() || record.summary.is_some() {
            return Err(IncidentDecisionError::WrongPhase);
        }
        if record.clean_retirement {
            return Ok(IncidentEffects::default());
        }
        if !record.seed_committed || record.pending.is_some() {
            return Err(IncidentDecisionError::WrongPhase);
        }
        let scope = record.seed_scope.ok_or(IncidentDecisionError::WrongPhase)?;
        let effects = IncidentEffects::one(IncidentEffect::RetireSource {
            scope: IncidentSourceScope::Live(scope),
        });
        let record = self.record_mut(index)?;
        record.clean_retirement = true;
        record.source_requested = true;
        record.family_settled = true;
        record.phase = IncidentServicePhase::Retiring;
        Ok(effects)
    }
    /// Accept checked clean-seed removal independently of the requested retirement.
    pub(super) fn seed_retired(
        &mut self,
        scope: IncidentOperationScope,
    ) -> Result<IncidentEffects, IncidentDecisionError> {
        let index = self.index(scope.incident())?;
        let record = self.record(index)?;
        if record.seed_scope != Some(scope) {
            return Err(IncidentDecisionError::StaleOperation);
        }
        if !record.clean_retirement || record.failure.is_some() || record.summary.is_some() {
            return Err(IncidentDecisionError::WrongPhase);
        }
        let record = self.record_mut(index)?;
        record.sources_retired = true;
        record.phase = IncidentServicePhase::Settled;
        Ok(IncidentEffects::default())
    }
    /// Accept exact already-issued source settlement, including after recovery closes.
    pub(super) fn sources_retired(
        &mut self,
        scope: IncidentSourceScope,
    ) -> Result<IncidentEffects, IncidentDecisionError> {
        let index = self.index(scope.incident())?;
        let record = self.record(index)?;
        if record.source_scope != scope {
            return Err(IncidentDecisionError::StaleOperation);
        }
        if !record.source_requested || record.clean_retirement {
            return Err(IncidentDecisionError::WrongPhase);
        }
        self.record_mut(index)?.sources_retired = true;
        self.settlement_effect(index)
    }
    /// Latch exact live or recovered cleanup failure without fabricating settlement.
    pub(super) fn source_retirement_failed(
        &mut self,
        scope: IncidentSourceScope,
    ) -> Result<IncidentEffects, IncidentDecisionError> {
        let index = self.index(scope.incident())?;
        let record = self.record(index)?;
        if record.source_scope != scope || !record.source_requested {
            return Err(IncidentDecisionError::StaleOperation);
        }
        if record.source_failed {
            return Ok(IncidentEffects::default());
        }
        if record.sources_retired {
            return Err(IncidentDecisionError::WrongPhase);
        }
        let record = self.record_mut(index)?;
        record.source_failed = true;
        record.latch_failure(IncidentServiceFailure::SourceRetirement);
        Ok(IncidentEffects::default())
    }
    /// Accept actual settlement for the exact desktop family, independently of exit.
    pub(super) fn family_settled(
        &mut self,
        run: RunKey,
    ) -> Result<IncidentEffects, IncidentDecisionError> {
        let index = self.index(IncidentKey::new(IncidentSubject::Desktop(run)))?;
        self.record_mut(index)?.family_settled = true;
        self.settlement_effect(index)
    }
    /// Accept exact reporter teardown independently of acknowledgement and source cleanup.
    pub(super) fn reporter_retired(
        &mut self,
        binding: ReportBinding,
    ) -> Result<IncidentEffects, IncidentDecisionError> {
        let index = self.index(binding.incident())?;
        if self.record(index)?.binding != Some(binding) {
            return Err(IncidentDecisionError::StaleOperation);
        }
        let record = self.record_mut(index)?;
        record.reporter_retired = true;
        record.presented = false;
        record.refresh_pending = false;
        if record.pending.is_none()
            && let Some(effect) = self.activate_queued_attempt(index)?
        {
            return Ok(IncidentEffects::one(effect));
        }
        self.settlement_effect(index)
    }
    /// Accept full-key supervisor family settlement from its actual family owner.
    pub(super) fn supervisor_family_settled(
        &mut self,
        owner: sugarsubstitute_contracts::lifecycle::CoordinatorKey,
    ) -> Result<IncidentEffects, IncidentDecisionError> {
        let index = self.index(IncidentKey::new(IncidentSubject::Supervisor(owner)))?;
        self.record_mut(index)?.family_settled = true;
        self.settlement_effect(index)
    }
    /// Forward restart intent once after actual acknowledgement, source and family proof.
    fn settlement_effect(
        &mut self,
        index: usize,
    ) -> Result<IncidentEffects, IncidentDecisionError> {
        let record = self.record(index)?;
        let effects = if record.restart_requested
            && !record.restart_forwarded
            && record.sources_retired
            && record.family_settled
        {
            match (record.seed.key().subject(), record.acknowledgement) {
                (IncidentSubject::Desktop(run), Some(acknowledgement)) => {
                    IncidentEffects::one(IncidentEffect::ForwardRestartIntent {
                        run,
                        acknowledgement,
                    })
                }
                _ => IncidentEffects::default(),
            }
        } else {
            IncidentEffects::default()
        };
        let forwarded = !effects.is_empty();
        let record = self.record_mut(index)?;
        record.restart_forwarded |= forwarded;
        if record.reclaimable() {
            record.phase = IncidentServicePhase::Settled;
        }
        Ok(effects)
    }
}
