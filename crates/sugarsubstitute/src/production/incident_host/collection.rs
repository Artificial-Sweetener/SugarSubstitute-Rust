// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Move the exact funded coordinator command into one real original collection worker.
use super::super::diagnostic_helper::collection::CollectionInput;
use super::*;
use sugarsubstitute_crash::{FinishedCaptureSource, incident::HostFailureFact};
use sugarsubstitute_supervision::diagnostic_storage::ProtectedDiagnosticWriteRequest;
/// Borrow the genuine retained sources and separately injected occurrence timestamp.
pub(in crate::production) struct CollectionSources<'a> {
    pub(in crate::production) request: &'a mut Option<ProtectedDiagnosticWriteRequest>,
    pub(in crate::production) capture: &'a mut Option<FinishedCaptureSource>,
    pub(in crate::production) occurred: IncidentUtc,
    pub(in crate::production) host_failure: &'a mut Option<HostFailureFact>,
}
impl IncidentHost {
    /// Borrow exact checked restoration of passive input that never entered native work.
    /// This fact grants no file/native cleanup or SourcesRetired proof.
    pub(in crate::production) fn passive_collection_restored_scope(
        &self,
    ) -> Option<IncidentOperationScope> {
        match self.completion.as_ref().map(|done| &done.outcome) {
            Some(JobOutcome::Success(value)) => value.policy.collection.passive_restored_scope(),
            _ => None,
        }
    }
    /// Restore the identical never-admitted policy and preserve its actual rejection.
    /// Dropping unissued passive loans creates no native/file retirement observation.
    pub(in crate::production) fn abandon_unsubmitted_collection(
        &mut self,
    ) -> Result<IncidentOperationScope, HostIssue> {
        let controller = match self.completion.as_mut().map(|done| &mut done.outcome) {
            Some(JobOutcome::Success(value)) => value,
            _ => return Err(HostIssue::Unprepared),
        };
        let policy = &mut controller.policy;
        let scope = policy
            .collection
            .abandon_unsubmitted(&mut policy.state)
            .map_err(|_| HostIssue::Pending)?;
        self.observe_passive_collection_restoration()?;
        Ok(scope)
    }
    /// Borrow the original admission cause after checked restoration of never-issued custody.
    pub(in crate::production) fn unsubmitted_collection_failure(
        &self,
    ) -> Option<&(dyn std::error::Error + 'static)> {
        match self.completion.as_ref().map(|done| &done.outcome) {
            Some(JobOutcome::Success(value)) => value.policy.collection.unsubmitted_failure(),
            _ => None,
        }
    }
    /// Observe only the original collection completion retained by its real service.
    pub(in crate::production) fn collection_outcome(&self) -> Option<TerminalKind> {
        match self.completion.as_ref().map(|done| &done.outcome) {
            Some(JobOutcome::Success(value)) => value.policy.collection.outcome(),
            _ => None,
        }
    }
    /// Observe only independent checked cleanup's real central terminal.
    pub(in crate::production) fn collection_cleanup_outcome(&self) -> Option<TerminalKind> {
        match self.completion.as_ref().map(|done| &done.outcome) {
            Some(JobOutcome::Success(value)) => value.policy.collection.cleanup_outcome(),
            _ => None,
        }
    }
    /// Categorize retained original failure after file cleanup without releasing its source.
    pub(in crate::production) fn collection_failed(&self) -> bool {
        match self.completion.as_ref().map(|done| &done.outcome) {
            Some(JobOutcome::Success(value)) => {
                let service = &value.policy.collection;
                let failed = service.failed();
                if failed {
                    let diagnostic = service.inspect(|operation| operation.diagnostic());
                    tracing::error!(
                        operation = "incident_collection_stopped",
                        worker_terminal = ?service.outcome(),
                        stage = ?diagnostic.map(|value| value.stage),
                        store = ?diagnostic.map(|value| value.store),
                        storage = ?diagnostic.map(|value| value.storage),
                        native_stage = ?diagnostic.map(|value| value.native_stage),
                        native_win32_code = ?diagnostic.map(|value| value.native_win32_code),
                        report = ?diagnostic.map(|value| value.report),
                        report_clock_failed = ?diagnostic.map(|value| value.report_clock_failed),
                        journal = ?diagnostic.map(|value| value.journal),
                        journal_storage = ?diagnostic.map(|value| value.journal_storage),
                        journal_post_panicked = ?diagnostic.map(|value| value.journal_post_panicked),
                        cleanup = ?diagnostic.map(|value| value.cleanup),
                        decision = ?diagnostic.map(|value| value.decision),
                        memory_requested_bytes = ?diagnostic.map(|value| value.memory_requested_bytes),
                        memory_available_bytes = ?diagnostic.map(|value| value.memory_available_bytes),
                        committed = ?diagnostic.map(|value| value.committed),
                        retired = ?diagnostic.map(|value| value.retired),
                        original_cause_retained = service
                            .inspect(|operation| operation.failure().is_some())
                            .unwrap_or(false),
                        "Original collection terminal and source custody retained"
                    );
                }
                failed
            }
            _ => true,
        }
    }
    /// Transfer only the pre-admitted Reporter metadata role; it grants no process authority.
    pub(in crate::production) fn reporter_funding(
        &mut self,
    ) -> Result<&mut Option<super::super::reporter_role::parent::ReporterFunding>, HostIssue> {
        if self.closing {
            return Err(HostIssue::Closed);
        }
        match self.completion.as_mut().map(|done| &mut done.outcome) {
            Some(JobOutcome::Success(value)) => Ok(&mut value.policy.reporter_funding),
            _ => Err(HostIssue::Unprepared),
        }
    }

    /// Submit the actual queued collection command with genuine retained namespace/capture sources.
    /// Admission denial retains the command and funded controller in the same source cell.
    pub(in crate::production) fn submit_collection(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
        until: Moment,
        sources: CollectionSources<'_>,
    ) -> Result<(), HostIssue> {
        let CollectionSources { request, capture, occurred, host_failure } = sources;
        if self.closing {
            return Err(HostIssue::Closed);
        }
        let controller = match self.completion.as_mut().map(|done| &mut done.outcome) {
            Some(JobOutcome::Success(value)) => value,
            _ => return Err(HostIssue::Unprepared),
        };
        let policy = &mut controller.policy;
        let state = policy.state.as_mut().ok_or(HostIssue::Pending)?;
        let index =
            state.state.effects.iter().position(Option::is_some).ok_or(HostIssue::Pending)?;
        let Some(IncidentEffect::CollectEvidence { scope: original, seed, .. }) =
            &state.state.effects[index]
        else {
            return Err(HostIssue::Identity);
        };
        if request.is_none()
            || capture.is_none()
            || until > original.original().until()
            || occurred < seed.created()
        {
            return Err(HostIssue::Identity);
        }
        let mut input = Some(CollectionInput {
            request: request.take().ok_or(HostIssue::Identity)?,
            capture: capture.take().ok_or(HostIssue::Identity)?,
            scope: *original,
            seed: seed.clone(),
            occurred,
            host_failure: host_failure.take(),
        });
        let effect = state.state.effects[index].take();
        let submitted =
            policy.collection.submit(client, scope, jobs, until, &mut input, &mut policy.state);
        if let Some(input) = input {
            *request = Some(input.request);
            *capture = Some(input.capture);
            *host_failure = input.host_failure;
            if let Some(state) = &mut policy.state {
                state.state.effects[index] = effect;
            }
        }
        submitted.map_err(|_| HostIssue::Pending)
    }
    /// Retry genuine central admission using only the input/controller already retained there.
    pub(in crate::production) fn retry_collection(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
        until: Moment,
    ) -> Result<(), HostIssue> {
        let controller = match self.completion.as_mut().map(|done| &mut done.outcome) {
            Some(JobOutcome::Success(value)) => value,
            _ => return Err(HostIssue::Unprepared),
        };
        controller
            .policy
            .collection
            .retry(client, scope, jobs, until)
            .map_err(|_| HostIssue::Pending)
    }
    /// Restore the exact funded coordinator only after matched original terminal delivery.
    pub(in crate::production) fn capture_collection(
        &mut self,
        batch: &CompletionBatch,
    ) -> Result<bool, HostIssue> {
        let controller = match self.completion.as_mut().map(|done| &mut done.outcome) {
            Some(JobOutcome::Success(value)) => value,
            _ => return Ok(false),
        };
        let policy = &mut controller.policy;
        let captured =
            policy.collection.capture(batch, &mut policy.state).map_err(|_| HostIssue::Identity)?;
        self.observe_passive_collection_restoration()?;
        Ok(captured)
    }
    /// Begin separately admitted close-only work under the original parent's retained cutoff.
    pub(in crate::production) fn submit_collection_cleanup(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
        until: Moment,
    ) -> Result<(), HostIssue> {
        let controller = match self.completion.as_mut().map(|done| &mut done.outcome) {
            Some(JobOutcome::Success(value)) => value,
            _ => return Err(HostIssue::Unprepared),
        };
        controller
            .policy
            .collection
            .submit_cleanup(client, scope, jobs, until)
            .map_err(|_| HostIssue::Pending)
    }
    /// Observe actual cleanup independently of collection/publication and parent-family settlement.
    pub(in crate::production) fn capture_collection_cleanup(
        &mut self,
        batch: &CompletionBatch,
    ) -> Result<bool, HostIssue> {
        let controller = match self.completion.as_mut().map(|done| &mut done.outcome) {
            Some(JobOutcome::Success(value)) => value,
            _ => return Ok(false),
        };
        controller.policy.collection.capture_cleanup(batch).map_err(|_| HostIssue::Identity)
    }
    /// Observe only the exact source scope whose genuine native and file cleanup completed.
    /// Keep other queued policy commands intact; this transition emits at most one restart intent.
    pub(in crate::production) fn adopt_collection_cleanup(&mut self) -> Result<(), HostIssue> {
        let controller = match self.completion.as_mut().map(|done| &mut done.outcome) {
            Some(JobOutcome::Success(value)) => value,
            _ => return Err(HostIssue::Unprepared),
        };
        let policy = &mut controller.policy;
        let scope = policy.collection.completed_scope().ok_or(HostIssue::Pending)?;
        let state = policy.state.as_mut().ok_or(HostIssue::Pending)?;
        if state.state.effects.iter().all(Option::is_some) {
            return Err(HostIssue::Pending);
        }
        let effects = state
            .state
            .coordinator
            .observe(IncidentServiceObservation::SourcesRetired {
                scope: IncidentSourceScope::Live(scope),
            })
            .map_err(|cause| {
                self.cause.get_or_insert(HostCause::Decision(cause));
                HostIssue::Decision
            })?;
        for (slot, effect) in
            state.state.effects.iter_mut().filter(|slot| slot.is_none()).zip(effects)
        {
            *slot = Some(effect);
        }
        policy.collection.release_adopted().map_err(|_| HostIssue::Pending)
    }
    /// Observe actual native/file cleanup; no process or report acknowledgement is inferred.
    pub(in crate::production) fn collection_files_retired(&self) -> bool {
        match self.completion.as_ref().map(|done| &done.outcome) {
            Some(JobOutcome::Success(value)) => value.policy.collection.files_retired(),
            _ => false,
        }
    }
}
