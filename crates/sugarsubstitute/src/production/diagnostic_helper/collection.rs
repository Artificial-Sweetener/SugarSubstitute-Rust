// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep collection, exact policy publication and physical source disposition in one finite worker.
use sugarsubstitute_application::incident::*;
use sugarsubstitute_contracts::{execution::*, incident::*};
use sugarsubstitute_crash::{FinishedCaptureSource, incident::*};
use sugarsubstitute_execution::ExecutionClient;
use sugarsubstitute_supervision::diagnostic_storage::*;

/// Consume only the original coordinator-issued collection command and genuine namespace source.
pub(in crate::production) struct CollectionInput {
    pub(in crate::production) request: ProtectedDiagnosticWriteRequest,
    pub(in crate::production) capture: FinishedCaptureSource,
    pub(in crate::production) scope: IncidentOperationScope,
    pub(in crate::production) seed: IncidentSeed,
    pub(in crate::production) occurred: IncidentUtc,
}
/// Keep every partial output and genuine cause before releasing any original claim.
pub(in crate::production) struct CollectionOperation {
    repository: Option<IncidentRepository>,
    opening: Option<DiagnosticStoreOpening>,
    ticket: Option<PublishTicket>,
    committed: Option<IncidentCommit>,
    first: Option<IncidentStoreError>,
    decision: Option<IncidentDecisionError>,
    retirement: Option<IncidentStoreError>,
    memory: Option<MemoryClaimError>,
    scope: Option<IncidentOperationScope>,
    source_settled: bool,
    retired: bool,
    report: Option<CapturedReport>,
}
impl CollectionOperation {
    pub(super) const fn new() -> Self {
        Self {
            repository: None,
            opening: None,
            ticket: None,
            committed: None,
            first: None,
            decision: None,
            retirement: None,
            memory: None,
            scope: None,
            source_settled: false,
            retired: false,
            report: None,
        }
    }
    /// Run with the actual funded coordinator inside the same finite admitted Storage job.
    /// All resulting commands stay in the caller's bounded controller effect slots.
    pub(super) fn execute(
        &mut self,
        input: CollectionInput,
        coordinator: &mut IncidentCoordinator,
        effects: &mut [Option<IncidentEffect>; MAX_INCIDENT_EFFECTS],
        context: &JobContext,
    ) {
        if self.scope.is_some() || effects.iter().any(Option::is_some) {
            self.first.get_or_insert(IncidentStoreError::RecoveryConflict);
            return;
        }
        self.scope = Some(input.scope);
        let result = self.collect(input, coordinator, effects, context);
        if let Err(cause) = result {
            self.first = Some(cause);
        }
    }
    fn collect(
        &mut self,
        input: CollectionInput,
        coordinator: &mut IncidentCoordinator,
        effects: &mut [Option<IncidentEffect>; MAX_INCIDENT_EFFECTS],
        context: &JobContext,
    ) -> Result<(), IncidentStoreError> {
        if context.spec().reservation_bytes != DIAGNOSTIC_STORAGE_BYTES
            || context.deadline().is_none_or(|until| until > input.scope.original().until())
            || input.seed.key() != input.scope.incident()
            || input.seed.revision() != input.scope.expected_revision()
            || input.occurred < input.seed.created()
        {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        let memory = context.memory().claim(DIAGNOSTIC_STORAGE_BYTES).map_err(|cause| {
            self.memory = Some(cause);
            IncidentStoreError::Capacity
        })?;
        let mut storage = DiagnosticStorage::new(memory)?;
        let operation = storage.begin_operation(input.scope.operation(), context)?;
        self.repository = Some(IncidentRepository::new(storage, operation, context)?);
        let repository = self.repository.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?;
        self.opening = Some(repository.resolve_namespace(&input.request, context)?);
        if !matches!(self.opening, Some(DiagnosticStoreOpening::Existing)) {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        repository.acquire_writer(context)?;
        repository.validate_store(context)?;
        repository.prepare_finished(&input.capture, &mut self.report, context)?;
        let report = self.report.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?;
        let capture = report.open(input.seed.key(), input.seed.build(), None, context)?;
        let bytes = match capture {
            CaptureEvidence::Finished(_) => {
                report.dump().ok_or(IncidentStoreError::MissingPayload)?.1
            }
            CaptureEvidence::Unavailable(_) => 0,
            CaptureEvidence::Awaiting => return Err(IncidentStoreError::RecoveryConflict),
        };
        let headroom = if bytes == 0 {
            131072
        } else {
            report.prepare_disposition(context)?;
            report.publication_headroom().ok_or(IncidentStoreError::RecoveryConflict)?
        };
        let preflight = repository.preflight_publication(
            input.seed,
            ExpectedSummary::Absent,
            IncidentPublicationDemand::new(bytes, usize::from(bytes != 0), 1, headroom)?,
            input.occurred,
            context,
        )?;
        self.ticket =
            Some(repository.begin_publish(preflight, input.scope.operation().id(), context)?);
        let ticket = self.ticket.as_mut().ok_or(IncidentStoreError::RecoveryConflict)?;
        let payload = if bytes == 0 {
            PayloadDisposition::Unavailable
        } else {
            let fact = repository.append_finished(ticket, report, context)?;
            PayloadDisposition::Retained(AttachmentInventory::new(&[fact])?)
        };
        let produced = coordinator
            .observe(IncidentServiceObservation::EvidenceCollected {
                scope: input.scope,
                evidence: CollectedIncidentEvidence::new(
                    input.occurred,
                    capture,
                    payload,
                    SymbolOutcome::NotRequested,
                ),
            })
            .map_err(|cause| {
                self.decision = Some(cause);
                IncidentStoreError::RecoveryConflict
            })?;
        for (slot, effect) in effects.iter_mut().zip(produced) {
            *slot = Some(effect);
        }
        let slot = effects
            .iter_mut()
            .find(|slot| slot.is_some())
            .ok_or(IncidentStoreError::RecoveryConflict)?;
        let Some(IncidentEffect::PublishSummary { scope, summary }) = slot.as_ref() else {
            return Err(IncidentStoreError::RecoveryConflict);
        };
        if *scope != input.scope {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        // The coordinator owns the exact expected content; storage independently verifies all files.
        self.committed = Some(repository.commit(ticket, summary.clone(), context)?);
        *slot = None;
        let committed = self.committed.as_ref().ok_or(IncidentStoreError::RecoveryConflict)?;
        if bytes != 0 {
            repository.publish_capture_journal(report, committed, context)?;
            repository.settle_capture_journal(report, context)?;
            self.source_settled = report.disposition_settled();
        }
        if effects.iter().any(Option::is_some) {
            return Err(IncidentStoreError::RecoveryConflict);
        }
        let produced = coordinator
            .observe(IncidentServiceObservation::SummaryCommitted {
                scope: input.scope,
                summary: committed.summary().clone(),
            })
            .map_err(|cause| {
                self.decision = Some(cause);
                IncidentStoreError::RecoveryConflict
            })?;
        for (slot, effect) in effects.iter_mut().zip(produced) {
            *slot = Some(effect);
        }
        Ok(())
    }
    /// Checked native token/source then repository cleanup on the genuine original close-only job.
    pub(super) fn retire(&mut self, client: &ExecutionClient, context: &JobContext) -> bool {
        if self.retired {
            return true;
        }
        let result = (|| {
            if let Some(report) = &mut self.report {
                report.retire(client, context)?;
            }
            if let Some(repository) = &mut self.repository {
                repository.retire(context)?;
            }
            Ok(())
        })();
        match result {
            Ok(()) => {
                self.retired = true;
                true
            }
            Err(cause) => {
                if self.retirement.is_none() {
                    self.retirement = Some(cause);
                }
                false
            }
        }
    }
    pub(in crate::production) fn failure(&self) -> Option<&(dyn std::error::Error + 'static)> {
        if let Some(report) = &self.report
            && let Some(cause) = report
                .journal_failure()
                .or_else(|| report.clock_failure())
                .or_else(|| report.failure())
        {
            return Some(cause);
        }
        if let Some(cause) = &self.memory {
            return Some(cause);
        }
        if let Some(cause) = &self.first {
            return Some(cause);
        }
        if let Some(cause) = &self.decision {
            return Some(cause);
        }
        self.retirement.as_ref().map(|cause| cause as _)
    }
    pub(super) const fn scope(&self) -> Option<IncidentOperationScope> {
        self.scope
    }
    pub(super) fn releasable(&self) -> bool {
        self.retired
            && self.committed.is_some()
            && self.first.is_none()
            && self.decision.is_none()
            && self.retirement.is_none()
            && self.memory.is_none()
    }
    pub(super) const fn files_retired(&self) -> bool {
        self.retired
    }
}

#[path = "collection/service.rs"]
mod service;
#[path = "collection/source.rs"]
mod source;
#[cfg(test)]
pub(in crate::production) use service::CollectionIssue;
pub(in crate::production) use service::CollectionService;
pub(in crate::production) use source::COLLECTION_SOURCE_BYTES;

#[cfg(test)]
mod tests {
    use super::*;
    use sugarsubstitute_contracts::lifecycle::*;
    use sugarsubstitute_supervision::diagnostic_storage::fixture::*;
    #[test]
    fn collection_cleanup_exposes_genuine_first_close_cause_after_later_checked_retirement()
    -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        crate::production::incident_host::tests::admitted_value(
            DIAGNOSTIC_STORAGE_BYTES,
            |context| {
                let fixture = DiagnosticStorageFixture::new()?;
                let mut storage = fixture.storage(&context)?;
                let owner = CoordinatorKey {
                    namespace: NamespaceId::new([1; 32]),
                    epoch: CoordinatorEpoch::new([2; 16])?,
                };
                let operation = storage.begin_operation(
                    LogicalStoreOperationKey::new(owner, IncidentOperationId::new(1)?),
                    &context,
                )?;
                let opening = fixture.resolve_opening(&mut storage, &operation, &context).map_err(
                    |cause| std::io::Error::other(format!("fixture resolve: {cause:?}")),
                )?;
                storage.acquire_writer(&operation, &context)?;
                storage.inventory(&operation, &context)?;
                let mut collection = CollectionOperation::new();
                collection.repository =
                    Some(IncidentRepository::new(storage, operation, &context).map_err(
                        |cause| std::io::Error::other(format!("fixture repository: {cause:?}")),
                    )?);
                collection.opening = Some(opening);
                if let Some(DiagnosticStoreOpening::Created(genesis)) = &mut collection.opening {
                    collection
                        .repository
                        .as_mut()
                        .ok_or("no repository")?
                        .initialize(genesis, &context)?;
                }
                fixture.set_failure(DiagnosticFixtureFailure::PartialCreate, true);
                let seed = IncidentSeed::new(
                    IncidentKey::new(IncidentSubject::Supervisor(owner)),
                    IncidentRevision::new(1)?,
                    BuildIdentity::new("collection-close-fixture")?,
                    IncidentUtc::from_unix_seconds(100)?,
                    IncidentStage::Startup,
                )?;
                let publication = collection
                    .repository
                    .as_mut()
                    .ok_or("no repository")?
                    .publish_seed(seed, &context);
                assert!(publication.is_err());
                fixture.set_failure(DiagnosticFixtureFailure::Close, true);
                // With no capture token present, retirement performs only the genuine repository close.
                let retirement =
                    collection.repository.as_mut().ok_or("no repository")?.retire(&context);
                assert!(retirement.is_err());
                collection.retirement = retirement.err();
                let original = collection.failure().ok_or("cleanup cause lost")?;
                assert!(std::error::Error::source(original).is_some());
                let pointer = std::ptr::from_ref(original);
                fixture.set_failure(DiagnosticFixtureFailure::Close, false);
                collection.repository.as_mut().ok_or("no repository")?.retire(&context).map_err(
                    |cause| std::io::Error::other(format!("fixture final close: {cause:?}")),
                )?;
                assert_eq!(collection.failure().map(std::ptr::from_ref), Some(pointer));
                Ok(collection)
            },
        )
    }
}
