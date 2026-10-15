// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Preflight incident state before committing authoritative M1 admission markers.
use super::{
    state::{FailureInput, Pending, Record},
    *,
};
use crate::supervision::{FailedDesktopFact, ResourceCoordinator};
use sugarsubstitute_contracts::incident::*;

impl IncidentCoordinator {
    /// Admit an immutable current-owner seed before fallible initialization.
    /// Exact repeats emit no effects; changed content, capacity and ID failures
    /// reject before changing the M1 marker or any local record.
    pub fn seed(
        &mut self,
        resources: &mut ResourceCoordinator,
        seed: IncidentSeed,
        context: IncidentRequestContext,
    ) -> Result<IncidentEffects, IncidentDecisionError> {
        let key = seed.key();
        if key.subject().coordinator() != self.owner {
            return Err(IncidentDecisionError::ForeignNamespace);
        }
        if let Ok(index) = self.index(key) {
            let record = self.record(index)?;
            return if record.seed == seed && record.seed_context == Some(context) {
                Ok(IncidentEffects::default())
            } else {
                Err(IncidentDecisionError::ChangedDuplicate)
            };
        }
        if matches!(key.subject(), IncidentSubject::Supervisor(_)) && self.supervisor_seed {
            return Err(IncidentDecisionError::StaleOperation);
        }
        let index = self.free_slot()?;
        let scope = self.scope(key, seed.revision(), context.budget())?;
        let effects =
            IncidentEffects::one(IncidentEffect::PublishSeed { scope, seed: seed.clone() });
        let record = Record {
            seed,
            seed_context: Some(context),
            seed_scope: Some(scope),
            seed_committed: false,
            original_publication: Some(scope.operation()),
            source_scope: IncidentSourceScope::Live(scope),
            current: Some(scope),
            pending: Some(Pending::Seed),
            failure: None,
            failure_context: None,
            summary: None,
            recovery_projection: None,
            published_scope: None,
            last_commit: None,
            phase: IncidentServicePhase::SeedPublishing,
            binding: None,
            automatic_report_attempted: false,
            presented: false,
            attempted_session: false,
            last_attempt: None,
            queued_failure: None,
            refresh_pending: false,
            reporter_retired: true,
            sources_retired: false,
            source_requested: false,
            source_failed: false,
            family_settled: false,
            clean_retirement: false,
            acknowledgement: None,
            restart_requested: false,
            restart_forwarded: false,
            first_failure: None,
            report_admission_failure: None,
        };
        match key.subject() {
            IncidentSubject::Desktop(run) => resources.admit_incident_seed(run)?,
            IncidentSubject::Supervisor(_) => self.supervisor_seed = true,
        }
        self.next_operation = scope.operation().id().get();
        self.records[index] = Some(record);
        Ok(effects)
    }
    /// Admit only a sealed actual failed exit from the authoritative transaction.
    /// Collection and summary publication share one original operation interval.
    pub fn failed_desktop(
        &mut self,
        resources: &mut ResourceCoordinator,
        fact: FailedDesktopFact,
        failure: FailureEvidence,
        context: IncidentRequestContext,
    ) -> Result<IncidentEffects, IncidentDecisionError> {
        if fact.run().owner != self.owner {
            return Err(IncidentDecisionError::ForeignNamespace);
        }
        let key = IncidentKey::new(IncidentSubject::Desktop(fact.run()));
        let index = self.index(key)?;
        let input = FailureInput {
            fact: Some(fact),
            attribution: ExitAttribution::Desktop {
                kind: fact.attribution(),
                status: fact.actual_status(),
            },
            failure,
        };
        let record = self.record(index)?;
        if let Some(old) = &record.failure {
            return if *old == input && record.failure_context == Some(context) {
                Ok(IncidentEffects::default())
            } else {
                Err(IncidentDecisionError::ChangedDuplicate)
            };
        }
        if !record.seed_committed || record.pending.is_some() || record.clean_retirement {
            return Err(IncidentDecisionError::WrongPhase);
        }
        record.seed.revision().next()?;
        let scope = self.scope(key, record.seed.revision(), context.budget())?;
        let effects = IncidentEffects::one(IncidentEffect::CollectEvidence {
            scope,
            seed: record.seed.clone(),
            attribution: input.attribution,
            failure,
        });
        let record = self.records[index].as_mut().ok_or(IncidentDecisionError::StaleOperation)?;
        resources.admit_failed_incident(fact)?;
        // Every fallible calculation and owned effect is prepared before the marker.
        record.failure = Some(input);
        record.failure_context = Some(context);
        record.current = Some(scope);
        record.source_scope = IncidentSourceScope::Live(scope);
        record.original_publication = Some(scope.operation());
        record.pending = Some(Pending::Collection);
        record.phase = IncidentServicePhase::Collecting;
        self.next_operation = scope.operation().id().get();
        Ok(effects)
    }
}
