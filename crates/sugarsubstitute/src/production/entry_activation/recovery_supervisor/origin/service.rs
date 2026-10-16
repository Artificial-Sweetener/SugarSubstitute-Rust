// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Observe true publication and cleanup terminals before releasing held-carrier admission.
#[path = "service/cleanup.rs"]
mod cleanup;
#[path = "service/outcome.rs"]
mod outcome;
#[path = "service/terminal.rs"]
mod terminal;
use super::*;
use crate::job_sequence::JobSequence;
use outcome::RetainedOutcome;
type Work = OwnedJob<(), OriginIssue>;
type Completion = RetainedOutcome<()>;
type CleanupWork = OwnedJob<bool, OriginIssue>;
type CleanupCompletion = RetainedOutcome<bool>;
/// Sole public service owner; reads/writes run only in its actual finite execution jobs.
/// A ready value is one retained matching fact, never another native origin/opening authority.
pub(in crate::production) struct OriginPublication {
    job: Option<Work>,
    completion: Option<Completion>,
    terminal: Option<TerminalKind>,
    original: Option<(ExecutionId, ScopeId, JobSpec)>,
    admission: Option<AdmissionError>,
    identity: Option<IdentityError>,
    cleanup: Option<CleanupWork>,
    cleanup_completion: Option<CleanupCompletion>,
    cleanup_terminal: Option<TerminalKind>,
    cleanup_original: Option<(ExecutionId, ScopeId, JobSpec)>,
    cleanup_budget: Option<ShutdownBudget>,
    cleanup_admission: Option<AdmissionError>,
    identity_rejected: bool,
    cleanup_clock_failed: bool,
    cleanup_time: Option<LifecycleRejection>,
    closing: bool,
    released: bool,
    provider_failure: Option<OriginFailure>,
    provider_cleanup_failure: Option<OriginFailure>,
    source: source::Source,
}
impl OriginPublication {
    /// Exact static metadata peak retained inside the original next4MiB projection.
    pub(in crate::production) const fn required_storage_bytes() -> u64 {
        source::storage_bytes()
    }
    /// Retain all actual constructor inputs before any storage/provider effect or allocation.
    /// # Errors
    /// Returns identical charge/grant/source/request on preflight or genuine source rejection.
    #[expect(
        clippy::result_large_err,
        reason = "Preserve every exact original constructor input without another allocation"
    )]
    pub(in crate::production) fn new(
        claim: MemoryClaim,
        grant: RegisteredRecoveryGrant,
        capture: CaptureRootSource,
        request: ProtectedDiagnosticWriteRequest,
        operation: LogicalStoreOperationKey,
        at: sugarsubstitute_contracts::incident::IncidentUtc,
    ) -> Result<Self, OriginPublicationRejection> {
        let input = OriginPublicationInput { claim, grant, capture, request, operation, at };
        let checked = (|| {
            if input.claim.bytes() != Self::required_storage_bytes()
                || input.claim.reserved_bytes() != 4 * 1024 * 1024
            {
                return Err(OriginConstructionFailure::Storage);
            }
            if input.operation.owner() != input.grant.key().run().owner {
                return Err(OriginConstructionFailure::Identity);
            }
            input.grant.execution_id().map_err(OriginConstructionFailure::Grant)
        })();
        let execution = match checked {
            Ok(execution) => execution,
            Err(cause) => return Err(OriginPublicationRejection { cause, _retained_input: input }),
        };
        Ok(Self::from_source(source::Source::new(input, execution)))
    }
    /// Install actual source custody; source funding drops after all service data and cause aliases.
    pub(in crate::production::entry_activation::recovery_supervisor::origin) fn from_source(
        source: source::Source,
    ) -> Self {
        Self {
            job: None,
            completion: None,
            terminal: None,
            original: None,
            admission: None,
            identity: None,
            cleanup: None,
            cleanup_completion: None,
            cleanup_terminal: None,
            cleanup_original: None,
            cleanup_budget: None,
            cleanup_admission: None,
            identity_rejected: false,
            cleanup_clock_failed: false,
            cleanup_time: None,
            closing: false,
            released: false,
            provider_failure: None,
            provider_cleanup_failure: None,
            source,
        }
    }
    /// Submit once under the unchanged genuine30-second grant, preserving rejected input for cleanup.
    pub(in crate::production) fn submit(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
    ) -> Result<(), OriginIssue> {
        if self.closing || self.original.is_some() || self.job.is_some() {
            return Err(OriginIssue::Started);
        }
        if scope.owner_id() != self.source.owner()? {
            return Err(OriginIssue::Identity);
        }
        let until = self.source.until()?;
        let id = jobs.allocate().map_err(|cause| {
            self.identity = Some(cause);
            OriginIssue::Identity
        })?;
        let spec = JobSpec {
            id,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(until),
            reservation_bytes: DIAGNOSTIC_STORAGE_BYTES,
            replacement: None,
        };
        self.original = Some((scope.owner_id(), scope.id(), spec));
        let source = self.source.alias()?;
        match client
            .submit_blocking(scope, spec, move |context| source.loan(true)?.execute(&context))
        {
            Ok(job) => {
                self.job = Some(job);
                Ok(())
            }
            Err(cause) => {
                self.admission = Some(cause);
                Err(OriginIssue::Admission)
            }
        }
    }
    /// Record matching notices and consume only exact original typed outcomes.
    pub(in crate::production) fn capture(
        &mut self,
        batch: &CompletionBatch,
    ) -> Result<(), OriginIssue> {
        let mut first = None;
        if let Err(cause) = self.capture_work(batch) {
            first = Some(cause);
        }
        if let Err(cause) = self.capture_cleanup(batch) {
            first.get_or_insert(cause);
        }
        self.retain_provider_failures();
        first.map_or(Ok(()), Err)
    }
    /// Advance checked cleanup after actual work termination; never wait for a worker.
    /// True means actual publication success, canonical read and closed native/repository custody.
    pub(in crate::production) fn poll(
        &mut self,
        client: &ExecutionClient,
        cleanup_scope: &Scope,
        jobs: &JobSequence,
    ) -> Result<bool, OriginIssue> {
        if self.released {
            return Ok(true);
        }
        if self.job.is_some() {
            return Ok(false);
        }
        if self.original.is_none() && !self.closing {
            return Ok(false);
        }
        self.advance_cleanup(client, cleanup_scope, jobs)?;
        if self.identity_rejected {
            return Err(OriginIssue::Identity);
        }
        if !self.cleanup_succeeded() {
            if self.cleanup_completion.is_some() {
                return Err(OriginIssue::Cleanup);
            }
            return Ok(false);
        }
        if self
            .completion
            .as_ref()
            .is_some_and(|done| matches!(done.original.outcome, JobOutcome::Success(())))
            && !self.closing
        {
            self.source.compact()?;
            self.completion = None;
            self.cleanup_completion = None;
            self.released = true;
            return Ok(true);
        }
        Err(OriginIssue::Operation)
    }
    /// Admit creation only for the identical genuine source after successful durable publication/cleanup.
    pub(in crate::production) fn ready(
        &self,
        grant: &RegisteredRecoveryGrant,
    ) -> Result<bool, OriginReadyFailure> {
        if self.identity_rejected {
            return Err(OriginReadyFailure::State(OriginIssue::Identity));
        }
        if !self.released
            || self.closing
            || !self.source.matches(grant).map_err(OriginReadyFailure::State)?
        {
            return Ok(false);
        }
        grant.check().map_err(OriginReadyFailure::Grant)?;
        self.source.ready(grant).map_err(OriginReadyFailure::State)
    }
    /// Actual checked file settlement remains independent of original publication outcome.
    pub(in crate::production) fn files_retired(&self) -> bool {
        self.released || self.cleanup_succeeded()
    }
    /// Retain at most one original and one independent cleanup source alias, without allocation.
    fn retain_provider_failures(&mut self) {
        if self.provider_failure.is_none() {
            self.provider_failure = self.source.failure(false);
        }
        if self.provider_cleanup_failure.is_none() {
            self.provider_cleanup_failure = self.source.failure(true);
        }
    }
    /// Borrow the original provider, admission, identity or supervised outcome before cleanup.
    /// Source aliases keep the actual cause and original allocation alive through their final drop.
    pub(in crate::production) fn failure(&self) -> Option<&(dyn Error + 'static)> {
        self.provider_failure
            .as_ref()
            .map(|cause| cause as &dyn Error)
            .or_else(|| self.admission.as_ref().map(|cause| cause as &dyn Error))
            .or_else(|| {
                self.completion
                    .as_ref()
                    .filter(|done| !matches!(done.original.outcome, JobOutcome::Success(())))
                    .map(|cause| cause as &dyn Error)
            })
            .or_else(|| self.identity.as_ref().map(|cause| cause as &dyn Error))
            .or_else(|| self.cleanup_failure())
    }
    /// Independent cleanup causes never overwrite an earlier original publication failure.
    fn cleanup_failure(&self) -> Option<&(dyn Error + 'static)> {
        self.provider_cleanup_failure
            .as_ref()
            .map(|cause| cause as &dyn Error)
            .or_else(|| self.cleanup_admission.as_ref().map(|cause| cause as &dyn Error))
            .or_else(|| self.cleanup_time.as_ref().map(|cause| cause as &dyn Error))
            .or_else(|| {
                self.cleanup_completion
                    .as_ref()
                    .filter(|done| !matches!(done.original.outcome, JobOutcome::Success(true)))
                    .map(|cause| cause as &dyn Error)
            })
    }
}

#[cfg(test)]
#[path = "service/failure_tests.rs"]
mod failure_tests;
