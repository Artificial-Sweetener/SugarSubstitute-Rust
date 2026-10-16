// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain native file custody before admission and qualify only actual successful delivery.
use super::*;
use crate::native_claims::files::Handle;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use sugarsubstitute_contracts::execution::{
    AdmissionError, ExecutionError, ExecutionPort, JobClass, JobContext, JobId, JobOutcome,
    JobSpec, MemoryClaim, Moment, Operation, OwnedJob, Scope, ScopeId,
};
use sugarsubstitute_execution::ExecutionClient;

pub(super) struct Inspection {
    pub(super) handles: Vec<Handle>,
    pub(super) manifest: Option<Arc<FrontendSourceManifest>>,
    pub(super) claim: Arc<MemoryClaim>,
    pub(super) search: usize,
    pub(super) deadline: Moment,
}
pub(super) trait SourcePort: Send + Sync {
    fn inspect(
        &self,
        input: &FrontendInstallationInput,
        retained: &mut Inspection,
        context: &JobContext,
    ) -> Result<(), FrontendSourceError>;
    fn close(
        &self,
        retained: &mut Inspection,
        context: &JobContext,
    ) -> Result<(), FrontendSourceError>;
}
pub(super) struct Shared {
    pub(super) retained: Mutex<Option<Box<Inspection>>>,
    live: AtomicBool,
}
impl Drop for Shared {
    fn drop(&mut self) {
        self.live.store(false, Ordering::Release);
        let retained = self.retained.get_mut().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(value) = retained.take() {
            if value.handles.is_empty() && value.search == 0 {
                drop(value);
            } else {
                // Owner loss has no admitted native-close authority. Retain first, even if logging panics.
                let _retained = Box::leak(value);
                tracing::error!(
                    operation = "frontend_source_owner_loss",
                    "Retained frontend files require explicit admitted closure"
                );
            }
        }
    }
}

/// Keep the full partial installation transaction independent of job result disposal.
/// Drop retains unresolved file handles and their existing byte reservation.
pub struct InstalledFrontendOwner {
    input: Arc<FrontendInstallationInput>,
    pub(super) shared: Arc<Shared>,
    pub(super) port: Arc<dyn SourcePort>,
    submitted: Option<(ScopeId, JobSpec, Moment)>,
    terminal: Option<FrontendSourceError>,
    execution: Option<sugarsubstitute_contracts::execution::ExecutionId>,
    client: Option<ExecutionClient>,
    pub(super) pending: Option<OwnedJob<(), FrontendSourceError>>,
}
impl InstalledFrontendOwner {
    /// Construct inert ownership; no filesystem access or executor is created.
    pub fn new(input: FrontendInstallationInput) -> Self {
        Self {
            input: Arc::new(input),
            shared: Arc::new(Shared { retained: Mutex::new(None), live: AtomicBool::new(false) }),
            port: Arc::new(super::native::SystemPort),
            submitted: None,
            terminal: None,
            execution: None,
            client: None,
            pending: None,
        }
    }
    /// Match all four exact path spellings against this owner's retained input.
    /// This inert comparison borrows the original selection, copies no proof and
    /// neither inspects files nor establishes current native custody or readiness.
    pub fn matches_selection(&self, input: &FrontendInstallationInput) -> bool {
        self.input.same_selection(input)
    }
    /// Admit one finite installed-file inspection, retaining partial native outputs outside its result.
    /// Requires Readiness/InfrastructureLifecycle, a finite deadline and the exact source reservation.
    /// # Errors
    /// Rejects repeated inspection or invalid specification before any native effect.
    pub fn submit_inspection(
        &mut self,
        execution: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
    ) -> Result<JobId, AdmissionError> {
        validate_spec(scope, spec)?;
        if self.submitted.is_some() {
            return Err(invalid_spec("Inspection already admitted"));
        }
        let deadline = effective_deadline(scope, spec)?;
        let shared = self.shared.clone();
        let input = self.input.clone();
        let port = self.port.clone();
        let job = execution.submit_blocking(scope, spec, move |context| {
            checkpoint(&context)?;
            let claim = Arc::new(
                context
                    .memory()
                    .claim(FRONTEND_SOURCE_BYTES)
                    .map_err(|e| FrontendSourceError::new("source_memory", e))?,
            );
            let mut guard = crate::registry::lock(&shared.retained);
            *guard = Some(Box::new(Inspection {
                handles: Vec::new(),
                manifest: None,
                claim,
                search: 0,
                deadline: context
                    .deadline()
                    .ok_or_else(|| FrontendSourceError::invalid("source_deadline"))?,
            }));
            let retained =
                guard.as_mut().ok_or_else(|| FrontendSourceError::invalid("source_custody"))?;
            port.inspect(&input, retained, &context)?;
            checkpoint(&context)?;
            shared.live.store(true, Ordering::Release);
            Ok(())
        })?;
        self.submitted = Some((scope.id(), spec, deadline));
        self.execution = Some(scope.owner_id());
        self.client = Some(execution.clone());
        self.pending = Some(job);
        Ok(spec.id)
    }
    /// Qualify the exact delivered success and current deadline before exposing expected source facts.
    /// A cancelled, late, foreign or failed completion cannot create a lease.
    /// # Errors
    /// Preserves domain failures and actual stopped/panicked terminal custody. Only matched
    /// delivery exposes terminal metadata; no stopped result grants a lease or physical closure.
    pub fn take_inspection(
        &mut self,
    ) -> Result<Option<InstalledFrontendLease>, FrontendSourceError> {
        let Some(completion) = self.pending.as_mut().and_then(OwnedJob::try_take_completion) else {
            return Ok(None);
        };
        self.pending = None;
        let matched = self.submitted.is_some_and(|(scope, spec, _)| {
            completion.scope_id == scope && completion.spec == spec
        });
        if !matched {
            let cause = FrontendSourceError::terminal(completion, false);
            self.terminal = Some(cause.clone());
            return Err(cause);
        }
        match completion.outcome {
            JobOutcome::Success(()) => (),
            JobOutcome::Failure(cause) => return Err(cause),
            _ => {
                let cause = FrontendSourceError::terminal(completion, true);
                self.terminal = Some(cause.clone());
                return Err(cause);
            }
        }
        let guard = crate::registry::lock(&self.shared.retained);
        let retained =
            guard.as_ref().ok_or_else(|| FrontendSourceError::invalid("source_no_custody"))?;
        let execution = self
            .client
            .as_ref()
            .ok_or_else(|| FrontendSourceError::invalid("source_no_executor"))?;
        if execution.now().map_err(|e| FrontendSourceError::new("source_delivery_clock", e))?
            >= retained.deadline
        {
            return Err(FrontendSourceError::invalid("source_delivery_identity_or_deadline"));
        }
        let manifest = guard
            .as_ref()
            .and_then(|value| value.manifest.clone())
            .ok_or_else(|| FrontendSourceError::invalid("source_no_manifest"))?;
        Ok(Some(InstalledFrontendLease {
            shared: self.shared.clone(),
            manifest,
            _claim: retained.claim.clone(),
        }))
    }
    /// Retire leases before admitting explicit closure on the original execution owner.
    /// A valid close request permanently retires this generation even if queue admission fails.
    /// # Errors
    /// Rejects an invalid or foreign finite specification before native closure.
    pub fn submit_close(
        &self,
        execution: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
    ) -> Result<OwnedJob<(), FrontendSourceError>, AdmissionError> {
        validate_spec(scope, spec)?;
        if self.pending.is_some() || self.execution != Some(scope.owner_id()) {
            return Err(invalid_spec("Inspection pending or foreign execution owner"));
        }
        self.shared.live.store(false, Ordering::Release);
        let shared = self.shared.clone();
        let port = self.port.clone();
        let job = execution.submit_blocking(scope, spec, move |context| {
            checkpoint(&context)?;
            let mut guard = crate::registry::lock(&shared.retained);
            if let Some(value) = guard.as_mut() {
                port.close(value, &context)?;
            }
            *guard = None;
            Ok(())
        })?;
        Ok(job)
    }
}

/// Keep immutable expected bytes alongside their retained canonical file owners.
/// This read-only lease grants no backend stop, process launch or arbitrary HTTP authority.
#[derive(Clone)]
pub struct InstalledFrontendLease {
    shared: Arc<Shared>,
    manifest: Arc<FrontendSourceManifest>,
    pub(super) _claim: Arc<MemoryClaim>,
}
impl InstalledFrontendLease {
    /// Borrow all observed installed versions, paths, lengths and SHA-256 digests.
    pub fn manifest(&self) -> &FrontendSourceManifest {
        &self.manifest
    }
    /// Check that explicit native closure has not retired this generation.
    /// # Errors
    /// Rejects retired or incomplete native custody.
    pub fn validate_live(&self) -> Result<(), FrontendSourceError> {
        if self.shared.live.load(Ordering::Acquire) {
            Ok(())
        } else {
            Err(FrontendSourceError::invalid("source_lease_retired"))
        }
    }
}
pub(super) fn checkpoint(context: &JobContext) -> Result<(), FrontendSourceError> {
    if context.cancellation_requested() {
        return Err(FrontendSourceError::invalid("source_cancelled"));
    }
    if context.deadline().is_none_or(|deadline| context.now() >= deadline) {
        return Err(FrontendSourceError::invalid("source_deadline"));
    }
    Ok(())
}
fn effective_deadline(scope: &Scope, spec: JobSpec) -> Result<Moment, AdmissionError> {
    match (scope.spec().deadline, spec.deadline) {
        (Some(a), Some(b)) => Ok(a.min(b)),
        (Some(value), None) | (None, Some(value)) => Ok(value),
        (None, None) => Err(AdmissionError::DeadlineRequired),
    }
}
fn validate_spec(scope: &Scope, spec: JobSpec) -> Result<(), AdmissionError> {
    effective_deadline(scope, spec)?;
    if spec.operation != Operation::Readiness
        || spec.class != JobClass::InfrastructureLifecycle
        || spec.reservation_bytes != FRONTEND_SOURCE_BYTES
        || spec.replacement.is_some()
    {
        return Err(invalid_spec("Invalid source inspection specification"));
    }
    Ok(())
}
fn invalid_spec(message: &'static str) -> AdmissionError {
    AdmissionError::Unexpected(ExecutionError::new(
        Operation::Readiness,
        Box::new(std::io::Error::other(message)),
    ))
}
