// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit the actual failed desktop and transfer its genuine selected Capture source once.
#[path = "incident_collection/material.rs"]
mod material;
#[cfg(test)]
#[path = "incident_collection/tests.rs"]
mod tests;
use super::incident_flow::transactions::{DiagnosticTransaction, DiagnosticTransactionRole};
use super::*;
use crate::production::incident_host::{CollectionSources, HostIssue};
use material::FailurePreparation;
use sugarsubstitute_application::{incident::*, supervision::FailedDesktopFact};
use sugarsubstitute_contracts::{execution::Moment, incident::*};
use sugarsubstitute_crash::{FinishedCaptureSource, incident::HostFailureFact};
use sugarsubstitute_supervision::diagnostic_storage::ProtectedDiagnosticWriteRequest;

/// Keep the exact original native exit, finite preparation, source loans and first cause.
/// All inline control storage is covered by the containing Record's original projection claim.
pub(super) struct IncidentCollection {
    transaction: Option<DiagnosticTransaction>,
    preparation: FailurePreparation,
    fact: Option<FailedDesktopFact>,
    original: Option<IncidentOperationBudget>,
    scope: Option<IncidentOperationScope>,
    request: Option<ProtectedDiagnosticWriteRequest>,
    capture: Option<FinishedCaptureSource>,
    host_failure: Option<HostFailureFact>,
    installed: bool,
    submitted: bool,
    cleanup_until: Option<Moment>,
    cleanup_submitted: bool,
    adopted: bool,
    checked_cleanup: bool,
    admission_abandoned: bool,
    first: Option<Box<dyn Error>>,
}
impl IncidentCollection {
    /// Construct inert controls before actual registry observation or source selection.
    pub(super) const fn new() -> Self {
        Self {
            transaction: None,
            preparation: FailurePreparation::new(),
            fact: None,
            original: None,
            scope: None,
            request: None,
            capture: None,
            host_failure: None,
            installed: false,
            submitted: false,
            cleanup_until: None,
            cleanup_submitted: false,
            adopted: false,
            checked_cleanup: false,
            admission_abandoned: false,
            first: None,
        }
    }
    /// Freeze one accepted actual exit and one original failure interval; never renew repeats.
    fn observe(&mut self, fact: FailedDesktopFact, now: Moment) -> Result<()> {
        if let Some(original) = self.fact {
            return if original == fact {
                Ok(())
            } else {
                Err("Actual failed desktop fact changed".into())
            };
        }
        let original = match self.original {
            Some(original) => original,
            None => IncidentOperationBudget::new(now)?,
        };
        self.fact = Some(fact);
        self.original = Some(original);
        Ok(())
    }
    /// Return the original actual incident interval, independently of exit observation order.
    pub(super) const fn original_budget(&self) -> Option<IncidentOperationBudget> {
        self.original
    }
    /// Bind a first actual pipe-terminal interval before its matching registered root exit arrives.
    pub(super) fn retain_original_budget(
        &mut self,
        original: IncidentOperationBudget,
    ) -> Result<()> {
        if self.original.is_some_and(|held| held != original) {
            return Err("Original failed desktop interval changed".into());
        }
        self.original = Some(original);
        Ok(())
    }
    /// Feed this preparation's actual notices from the existing immutable global batch.
    pub(super) fn capture(&mut self, batch: &CompletionBatch) -> Result<()> {
        self.preparation.capture(batch)
    }
    /// Keep retirement fenced until actual checked cleanup or restoration of never-issued input.
    pub(super) fn pending(&self) -> bool {
        self.fact.is_some()
            && !self.adopted
            && !self.checked_cleanup
            && !self.admission_abandoned
            && (self.first.is_none() || self.installed)
    }
    /// Require actual checked source retirement and finite metadata delivery before family cleanup.
    pub(super) fn source_retired(&self) -> bool {
        self.checked_cleanup && self.preparation.delivered()
    }
    /// Borrow completed same-exit collection and checked original source disposal.
    /// This is a normal-idle veto query; it grants no cleanup or report authority.
    pub(super) fn completed_failed(&self, fact: FailedDesktopFact) -> bool {
        self.fact == Some(fact)
            && self.adopted
            && self.checked_cleanup
            && self.installed
            && self.submitted
            && !self.admission_abandoned
            && self.request.is_none()
            && self.capture.is_none()
            && self.preparation.delivered()
            && self.failure().is_none()
            && self.scope.is_some_and(|scope| {
                scope.incident() == IncidentKey::new(IncidentSubject::Desktop(fact.run()))
                    && self.original == Some(scope.original())
            })
    }
    /// Borrow original preparation/caller failure without replacing the repository's retained cause.
    pub(super) fn failure(&self) -> Option<&(dyn Error + 'static)> {
        self.preparation.failure().or(self.first.as_deref())
    }
    /// Cancel only finite metadata preparation; storage source retirement belongs to IncidentHost.
    pub(super) fn cancel_preparation(&self) {
        self.preparation.cancel();
    }
    /// Observe genuine metadata terminal delivery separately from native record retirement.
    pub(super) fn delivered(&self) -> bool {
        self.preparation.delivered()
    }
    /// Return the retained next original cutoff to the composing native wake owner.
    pub(super) fn next_deadline(&self) -> Option<Moment> {
        if !self.pending() {
            return None;
        }
        self.cleanup_until.or_else(|| self.original.map(IncidentOperationBudget::until))
    }
}

/// Advance actual failed collection even when the just-observed parent activity failed.
/// This method neither drains central completions nor performs protected file/provider effects.
pub(super) fn progress(
    record: &mut Record,
    incidents: &mut incident_flow::IncidentFlow,
    capture: &CaptureRegistration,
    backend: &mut CoordinatorBackend,
    resources: &EntryResources<'_>,
    closing: bool,
) -> Result<()> {
    let result = advance(record, incidents, capture, backend, resources, closing);
    if let Err(cause) = result
        && record.collection.first.is_none()
    {
        record.collection.first = Some(cause);
        return Err("Actual desktop collection failed with retained original source".into());
    }
    Ok(())
}
/// Serialize the one funded policy/source owner across real desktop generations.
fn advance(
    record: &mut Record,
    incidents: &mut incident_flow::IncidentFlow,
    capture: &CaptureRegistration,
    backend: &mut CoordinatorBackend,
    resources: &EntryResources<'_>,
    closing: bool,
) -> Result<()> {
    if record.collection.adopted
        || record.collection.checked_cleanup
        || record.collection.admission_abandoned
    {
        return Ok(());
    }
    if record.collection.fact.is_none() {
        if closing || !record.seeded || !incidents.host.prepared() {
            return Ok(());
        }
        let Some(parent) = &record.parent else {
            return Ok(());
        };
        let Some(process) =
            record.start.as_ref().and_then(|start| start.ticket).map(|ticket| ticket.process())
        else {
            return Ok(());
        };
        let Some(fact) = parent.failed_incident(record.run, process)? else {
            return Ok(());
        };
        if let Some(pending) = &record.pending_desktop_failure {
            record.collection.retain_original_budget(pending.original())?;
        }
        record.collection.observe(fact, resources.client.now()?)?;
    }
    let collection = &mut record.collection;
    if !collection.installed
        && collection.host_failure.is_none()
        && let Some(output) = record.start.as_ref().and_then(|start| start.output.as_ref())
    {
        collection.host_failure = output.host_failure()?;
    }
    if collection.first.is_some() && !collection.installed {
        return Ok(());
    }
    let fact = collection.fact.ok_or("Original failed desktop fact missing")?;
    let original = collection.original.ok_or("Original failed desktop cutoff missing")?;
    if let Some(restored) = incidents.host.passive_collection_restored_scope() {
        if collection.scope != Some(restored) || incidents.collection_run != Some(record.run) {
            return Err("Actual passive collection restoration identity changed".into());
        }
        incidents.transactions.retired(restored.operation(), &mut collection.transaction)?;
        collection.admission_abandoned = true;
        incidents.collection_run = None;
        return Err(
            "Original collection stopped before source execution; actual terminal retained".into(),
        );
    }
    if collection.installed && !collection.submitted && resources.client.now()? >= original.until()
    {
        let restored = incidents.host.abandon_unsubmitted_collection()?;
        if collection.scope != Some(restored) || incidents.collection_run != Some(record.run) {
            return Err("Actual unsubmitted collection restoration identity changed".into());
        }
        incidents.transactions.retired(restored.operation(), &mut collection.transaction)?;
        collection.admission_abandoned = true;
        incidents.collection_run = None;
        return Err(
            "Original collection admission failed; passive source restored without file effects"
                .into(),
        );
    }
    if !collection.installed && resources.client.now()? >= original.until() {
        collection.preparation.cancel();
        return Err("Original failed desktop admission interval elapsed".into());
    }
    collection.preparation.submit(fact, original, resources)?;
    let Some(material) = collection.preparation.material()? else {
        return Ok(());
    };
    if incidents.collection_run.is_some_and(|run| run != record.run) || incidents.seed_busy() {
        return Ok(());
    }
    if collection.scope.is_none() {
        if !incidents.transactions.available() || incidents.host.next_effect()?.is_some() {
            return Ok(());
        }
        incidents.host.failed_desktop(
            backend
                .incident_policy(
                    sugarsubstitute_application::supervision::ResourceStorageInput::Current,
                )
                .map_err(outer)?,
            material.fact,
            FailureEvidence::Unexplained,
            material.context,
        )?;
        let Some(IncidentEffect::CollectEvidence { scope, attribution, failure, .. }) =
            incidents.host.next_effect()?
        else {
            return Err("Actual failed desktop collection command missing".into());
        };
        if scope.incident() != IncidentKey::new(IncidentSubject::Desktop(record.run))
            || *attribution
                != (ExitAttribution::Desktop {
                    kind: fact.attribution(),
                    status: fact.actual_status(),
                })
            || *failure != FailureEvidence::Unexplained
            || scope.original() != original
        {
            return Err("Actual failed desktop collection command changed".into());
        }
        collection.scope = Some(*scope);
        incidents.collection_run = Some(record.run);
    }
    let scope = collection.scope.ok_or("Actual collection scope missing")?;
    if !collection.installed {
        if collection.request.is_none() {
            collection.request =
                Some(incidents.host.namespace()?.diagnostic_write_request(scope.operation())?);
        }
        if collection.capture.is_none() {
            let root =
                capture.root_source().ok_or("Actual retained Root storage source missing")?;
            collection.capture =
                Some(if let Some(grant) = record.start.as_ref().and_then(DesktopStart::grant) {
                    root.recovery_finished_source(grant)?
                } else {
                    root.finished_source()
                });
        }
        if !incidents.transactions.admit(
            DiagnosticTransactionRole::Collection,
            scope.operation(),
            &mut collection.transaction,
        )? {
            return Ok(());
        }
        let submitted = incidents.host.submit_collection(
            resources.client,
            resources.finite,
            resources.jobs,
            original.until(),
            CollectionSources {
                request: &mut collection.request,
                capture: &mut collection.capture,
                occurred: material.context.at(),
                host_failure: &mut collection.host_failure,
            },
        );
        if collection.request.is_none() && collection.capture.is_none() {
            collection.installed = true;
        } else if collection.request.is_some() && collection.capture.is_some() {
            incidents.transactions.retired(scope.operation(), &mut collection.transaction)?;
        } else {
            return Err("Actual collection source transfer split its original custody".into());
        }
        match submitted {
            Ok(()) => collection.submitted = true,
            Err(HostIssue::Pending) => return Ok(()),
            Err(cause) => return Err(cause.into()),
        }
    }
    if !collection.submitted {
        match incidents.host.retry_collection(
            resources.client,
            resources.finite,
            resources.jobs,
            original.until(),
        ) {
            Ok(()) => collection.submitted = true,
            Err(HostIssue::Pending) => return Ok(()),
            Err(cause) => return Err(cause.into()),
        }
    }
    if incidents.host.collection_outcome().is_none() {
        return Ok(());
    }
    if collection.cleanup_until.is_none() {
        let until = resources.client.now()?.checked_add(std::time::Duration::from_secs(15))?;
        collection.cleanup_until = Some(
            record
                .parent
                .as_ref()
                .and_then(|parent| parent.shutdown())
                .map_or(until, |budget| until.min(budget.settle_until)),
        );
    }
    if !collection.cleanup_submitted {
        match incidents.host.submit_collection_cleanup(
            resources.client,
            resources.cleanup,
            resources.jobs,
            collection.cleanup_until.ok_or("Original collection cleanup cutoff missing")?,
        ) {
            Ok(()) => collection.cleanup_submitted = true,
            Err(HostIssue::Pending) => return Ok(()),
            Err(cause) => return Err(cause.into()),
        }
    }
    if !incidents.host.collection_files_retired() {
        if incidents.host.collection_cleanup_outcome().is_some() {
            return Err("Original collection checked cleanup failed with retained source".into());
        }
        return Ok(());
    }
    incidents.transactions.retired(scope.operation(), &mut collection.transaction)?;
    if incidents.host.collection_failed() {
        collection.checked_cleanup = true;
        incidents.collection_run = None;
        return Err("Original collection worker/native cause retained after checked cleanup".into());
    }
    if let Some(IncidentEffect::RetireSource { scope: IncidentSourceScope::Live(actual) }) =
        incidents.host.next_effect()?
    {
        if *actual != scope {
            return Err("Original source retirement scope changed".into());
        }
        incidents.host.take_effect()?;
    } else {
        return Err("Actual checked source retirement command missing".into());
    }
    incidents.host.adopt_collection_cleanup()?;
    collection.adopted = true;
    collection.checked_cleanup = true;
    incidents.collection_run = None;
    Ok(())
}
