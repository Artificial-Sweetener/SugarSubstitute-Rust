// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Emit finite owned decisions for actual host adapters without performing effects.
use super::{
    IncidentOperationScope, IncidentRecoveryScope, IncidentSourceScope, MAX_INCIDENT_EFFECTS,
};
use sugarsubstitute_contracts::{incident::*, lifecycle::RunKey};

/// One host-coordinated use case; matching values alone grant no native capability.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IncidentEffect {
    /// Recover bounded stored incidents under a current admission.
    Recover {
        /// Current admitted recovery scope.
        scope: IncidentRecoveryScope,
    },
    /// Commit an immutable seed before fallible initialization.
    PublishSeed {
        /// Exact current original scope.
        scope: IncidentOperationScope,
        /// Exact seed bytes represented by owned facts.
        seed: IncidentSeed,
    },
    /// Collect evidence without changing accepted lifecycle classification.
    CollectEvidence {
        /// Original collection/publication scope.
        scope: IncidentOperationScope,
        /// Immutable original seed.
        seed: IncidentSeed,
        /// Accepted exit attribution.
        attribution: ExitAttribution,
        /// Originally admitted failure mechanism.
        failure: FailureEvidence,
    },
    /// Publish the exact prepared successor summary.
    PublishSummary {
        /// Original collection scope and predecessor revision.
        scope: IncidentOperationScope,
        /// Exact expected resulting content.
        summary: IncidentSummary,
    },
    /// Request the single automatic reporter attempt.
    LaunchReporter {
        /// New current reporter session.
        binding: ReportBinding,
        /// Actually committed summary.
        summary: IncidentSummary,
    },
    /// Persist an actual presentation/failure disposition once.
    RecordAttempt {
        /// Current CAS operation.
        scope: IncidentOperationScope,
        /// Session whose actual outcome is recorded.
        binding: ReportBinding,
        /// Actual outcome.
        attempt: ReportAttempt,
        /// Actual observation UTC.
        at: IncidentUtc,
    },
    /// Refresh the same session after an actual attempt CAS.
    ReplaceReportView {
        /// Previous view binding, invalidated immediately by CAS.
        old: ReportBinding,
        /// New exact committed view binding.
        current: ReportBinding,
        /// Actual committed content.
        summary: IncidentSummary,
    },
    /// Request exact durable acknowledgement; UI intent alone never closes the report.
    Acknowledge {
        /// Current acknowledgement CAS.
        scope: IncidentOperationScope,
        /// Exact currently visible session.
        binding: ReportBinding,
        /// Actual requested acknowledgement UTC.
        at: IncidentUtc,
    },
    /// Export after actual native save selection without acknowledging.
    Export {
        /// Current bounded export operation.
        scope: IncidentOperationScope,
        /// Exact visible view.
        binding: ReportBinding,
        /// Explicit user content selection.
        selection: ExportSelection,
    },
    /// Close only after accepting actual exact durable acknowledgement.
    CloseAfterDurableAck {
        /// Session to retire.
        binding: ReportBinding,
        /// Actual committed acknowledgement.
        acknowledgement: DurableAcknowledgement,
    },
    /// Forward recovered-report user intent after durable ACK and current reader retirement.
    /// The historical subject grants no process/run ownership. The composing current-root
    /// registered startup owner must separately close its actual current run and admit fresh launch.
    ForwardRecoveredRestartIntent {
        /// Immutable old incident whose visible report was explicitly acknowledged.
        incident: IncidentKey,
        /// Actual durable acknowledgement for that same historical incident.
        acknowledgement: DurableAcknowledgement,
    },
    /// Forward user intent after independent settlement; M1 retains launch authority.
    ForwardRestartIntent {
        /// Exact old desktop run.
        run: RunKey,
        /// Its actual durable acknowledgement.
        acknowledgement: DurableAcknowledgement,
    },
    /// Retire source custody under retained actual adapter cleanup authority.
    RetireSource {
        /// Original live or already-admitted recovered source correlation.
        scope: IncidentSourceScope,
    },
}
/// Fixed effect slots preserve the per-transition limit without dynamic growth.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IncidentEffects {
    items: Box<[Option<IncidentEffect>; MAX_INCIDENT_EFFECTS]>,
}
impl Default for IncidentEffects {
    fn default() -> Self {
        Self { items: Box::new(std::array::from_fn(|_| None)) }
    }
}
impl IntoIterator for IncidentEffects {
    type Item = IncidentEffect;
    type IntoIter =
        std::iter::Flatten<std::array::IntoIter<Option<IncidentEffect>, MAX_INCIDENT_EFFECTS>>;
    fn into_iter(self) -> Self::IntoIter {
        (*self.items).into_iter().flatten()
    }
}
impl IncidentEffects {
    /// Prepare one owned effect before any authoritative admission marker changes.
    pub(super) fn one(effect: IncidentEffect) -> Self {
        let mut result = Self::default();
        result.items[0] = Some(effect);
        result
    }
    /// Prepare ordered independent effects inside the fixed eight-slot inventory.
    pub(super) fn pair(first: IncidentEffect, second: IncidentEffect) -> Self {
        let mut result = Self::one(first);
        result.items[1] = Some(second);
        result
    }
    /// Prepare the ordered close, restart intent and queued failure inside fixed slots.
    pub(super) fn three(
        first: IncidentEffect,
        second: IncidentEffect,
        third: IncidentEffect,
    ) -> Self {
        let mut result = Self::pair(first, second);
        result.items[2] = Some(third);
        result
    }
    /// Preserve transition order inside the same original fixed effect allocation.
    /// # Errors
    /// Rejects a full original effect envelope without replacing an existing effect.
    pub(super) fn append(
        &mut self,
        effect: IncidentEffect,
    ) -> Result<(), super::IncidentDecisionError> {
        let slot = self
            .items
            .iter_mut()
            .find(|slot| slot.is_none())
            .ok_or(super::IncidentDecisionError::Saturated)?;
        *slot = Some(effect);
        Ok(())
    }
    /// Inspect the finite effects without executing them.
    pub fn iter(&self) -> impl Iterator<Item = &IncidentEffect> {
        self.items.iter().flatten()
    }
    /// Return actual emitted count, at most eight.
    pub fn len(&self) -> usize {
        self.iter().count()
    }
    /// Return whether no new effects were admitted.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
/// Local copy intent or finite independently admitted adapter effects.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IncidentReportDecision {
    /// Request bounded native clipboard handling without any store mutation.
    CopyText {
        /// Exact visible view.
        binding: ReportBinding,
    },
    /// Execute the scoped use cases through actual host adapters.
    Effects(IncidentEffects),
}
