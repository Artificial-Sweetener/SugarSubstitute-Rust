// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain admitted native HTTP outputs independently of cancellation and payload disposal.
use super::{types::*, *};
use std::sync::{Arc, Mutex};
use sugarsubstitute_contracts::execution::{
    AdmissionError, ExecutionError, ExecutionPort, JobClass, JobContext, JobId, JobOutcome,
    JobSpec, MemoryClaim, Moment, Operation, OwnedJob, Scope,
};
use sugarsubstitute_execution::ExecutionClient;
pub(super) struct Transaction {
    pub(super) session: usize,
    pub(super) connect: usize,
    pub(super) request: usize,
    pub(super) _claim: Arc<MemoryClaim>,
    pub(super) deadline: Moment,
    pub(super) peer: Option<sugarsubstitute_supervision::backend_listener::BackendPeerIdentity>,
}
pub(super) struct Shared {
    pub(super) transaction: Mutex<Option<Box<Transaction>>>,
}
impl Drop for Shared {
    fn drop(&mut self) {
        let state = self.transaction.get_mut().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(value) = state.take() {
            if value.session == 0 && value.connect == 0 && value.request == 0 {
                drop(value);
            } else {
                let _retained = Box::leak(value);
            }
        }
    }
}
pub(super) trait HttpPort: Send + Sync {
    fn begin(
        &self,
        state: &mut Transaction,
        port: u16,
        context: &JobContext,
    ) -> Result<(), ReadinessTransportError>;
    fn get(
        &self,
        state: &mut Transaction,
        input: &ReadinessProbeInput,
        endpoint: super::probe::Endpoint<'_>,
        context: &JobContext,
        consume: &mut dyn FnMut(&[u8]) -> Result<(), ReadinessTransportError>,
    ) -> Result<(), ReadinessTransportError>;
    fn close(
        &self,
        state: &mut Transaction,
        context: &JobContext,
    ) -> Result<(), ReadinessTransportError>;
}
/// Own one original-deadline HTTP attempt, including partial request/connection/session custody.
/// Dropping this owner performs no native closes and retains unresolved resources with their charge.
pub struct ReadinessProbeOwner {
    input: Arc<ReadinessProbeInput>,
    shared: Arc<Shared>,
    port: Arc<dyn HttpPort>,
    pending: Option<OwnedJob<(), ReadinessTransportError>>,
    submitted: bool,
    client: Option<ExecutionClient>,
    execution: Option<sugarsubstitute_contracts::execution::ExecutionId>,
}
impl ReadinessProbeOwner {
    /// Prepare inert retained custody before admission; this performs no HTTP or process query.
    pub fn new(input: ReadinessProbeInput) -> Self {
        Self {
            input: Arc::new(input),
            shared: Arc::new(Shared { transaction: Mutex::new(None) }),
            port: Arc::new(super::native::SystemPort),
            pending: None,
            submitted: false,
            client: None,
            execution: None,
        }
    }
    /// Submit one finite readiness attempt with immutable effective deadline.
    /// # Errors
    /// Rejects repeated attempts or invalid specifications before invoking native effects.
    pub fn submit_attempt(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
    ) -> Result<JobId, AdmissionError> {
        validate(spec)?;
        if self.submitted {
            return Err(invalid("Readiness attempt already submitted"));
        }
        let shared = self.shared.clone();
        let input = self.input.clone();
        let port = self.port.clone();
        let job = client.submit_blocking(scope, spec, move |context| {
            checkpoint(&context)?;
            let claim = Arc::new(
                context
                    .memory()
                    .claim(READINESS_TRANSPORT_BYTES - 128 * 1024)
                    .map_err(|e| ReadinessTransportError::new("attempt_memory", e))?,
            );
            let mut guard = lock(&shared.transaction);
            *guard = Some(Box::new(Transaction {
                session: 0,
                connect: 0,
                request: 0,
                _claim: claim,
                deadline: context
                    .deadline()
                    .ok_or_else(|| ReadinessTransportError::invalid("attempt_deadline"))?,
                peer: None,
            }));
            let state = guard
                .as_mut()
                .ok_or_else(|| ReadinessTransportError::invalid("attempt_custody"))?;
            port.begin(state, input.port(), &context)?;
            super::probe::inspect(port.as_ref(), state, &input, &context)?;
            port.close(state, &context)?;
            checkpoint(&context)
        })?;
        self.pending = Some(job);
        self.submitted = true;
        self.client = Some(client.clone());
        self.execution = Some(scope.owner_id());
        Ok(spec.id)
    }
    /// Consume this owner's actual completion and qualify fresh delivery before creating readiness proof.
    /// # Errors
    /// Rejects cancellation, failure, late delivery, missing peer or retired installed sources.
    pub fn take_readiness(&mut self) -> Result<Option<VerifiedReadiness>, ReadinessTransportError> {
        let Some(completion) = self.pending.as_mut().and_then(OwnedJob::try_take_completion) else {
            return Ok(None);
        };
        self.pending = None;
        match completion.outcome {
            JobOutcome::Success(()) => (),
            JobOutcome::Failure(cause) => return Err(cause),
            _ => return Err(ReadinessTransportError::invalid("attempt_terminal")),
        }
        let guard = lock(&self.shared.transaction);
        let state =
            guard.as_ref().ok_or_else(|| ReadinessTransportError::invalid("attempt_no_custody"))?;
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| ReadinessTransportError::invalid("attempt_no_executor"))?;
        if client.now().map_err(|e| ReadinessTransportError::new("attempt_delivery_clock", e))?
            >= state.deadline
            || state.request != 0
            || state.connect != 0
            || state.session != 0
        {
            return Err(ReadinessTransportError::invalid("attempt_delivery_or_retirement"));
        }
        self.input
            .source
            .validate_live()
            .map_err(|e| ReadinessTransportError::new("expected_source_retired", e))?;
        Ok(Some(VerifiedReadiness {
            input: self.input.clone(),
            peer: state.peer.ok_or_else(|| ReadinessTransportError::invalid("attempt_no_peer"))?,
        }))
    }
    /// Admit explicit partial cleanup on a finite shutdown deadline without resetting the attempt deadline.
    /// # Errors
    /// Rejects foreign execution or cleanup while the actual attempt remains pending.
    pub fn submit_close(
        &self,
        client: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
    ) -> Result<OwnedJob<(), ReadinessTransportError>, AdmissionError> {
        validate(spec)?;
        if self.pending.is_some() || self.execution != Some(scope.owner_id()) {
            return Err(invalid("Readiness pending or foreign cleanup owner"));
        }
        let shared = self.shared.clone();
        let input = self.input.clone();
        let port = self.port.clone();
        client.submit_blocking(scope, spec, move |context| {
            checkpoint(&context)?;
            let mut guard = lock(&shared.transaction);
            if let Some(state) = guard.as_mut() {
                port.close(state, &context)?;
            }
            if let Listener::Attached(lease, _) = &input.listener {
                lease
                    .close(&context)
                    .map_err(|e| ReadinessTransportError::new("attached_identity_close", e))?;
            }
            *guard = None;
            Ok(())
        })
    }
}
pub(super) fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}
pub(super) fn checkpoint(context: &JobContext) -> Result<(), ReadinessTransportError> {
    if context.cancellation_requested() {
        return Err(ReadinessTransportError::invalid("attempt_cancelled"));
    }
    if context.deadline().is_none_or(|deadline| context.now() >= deadline) {
        return Err(ReadinessTransportError::invalid("attempt_deadline"));
    }
    Ok(())
}
fn validate(spec: JobSpec) -> Result<(), AdmissionError> {
    if spec.deadline.is_none() {
        return Err(AdmissionError::DeadlineRequired);
    }
    if spec.operation != Operation::Readiness
        || spec.class != JobClass::InfrastructureLifecycle
        || spec.reservation_bytes != READINESS_TRANSPORT_BYTES
        || spec.replacement.is_some()
    {
        return Err(invalid("Invalid readiness specification"));
    }
    Ok(())
}
fn invalid(message: &'static str) -> AdmissionError {
    AdmissionError::Unexpected(ExecutionError::new(
        Operation::Readiness,
        Box::new(std::io::Error::other(message)),
    ))
}
#[cfg(test)]
#[path = "tests/ownership.rs"]
mod tests;
