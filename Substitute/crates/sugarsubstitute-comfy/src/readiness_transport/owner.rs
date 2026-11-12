// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retain admitted native HTTP outputs independently of cancellation and payload disposal.
use super::{types::*, *};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
#[cfg(test)]
use sugarsubstitute_contracts::execution::JobOutcome;
use sugarsubstitute_contracts::execution::{
    AdmissionError, ExecutionError, ExecutionPort, JobClass, JobContext, JobId, JobSpec, Moment,
    Operation, OwnedJob, Scope, ScopeId,
};
use sugarsubstitute_execution::ExecutionClient;
pub(super) struct Transaction {
    pub(super) http: sugarsubstitute_networking::Transport,
    #[cfg(test)]
    pub(super) fixture_slots: [usize; 3],
    pub(super) tcp: sugarsubstitute_supervision::backend_listener::TcpOwnerWorkspace,
    pub(super) deadline: Moment,
    pub(super) peer: Option<sugarsubstitute_supervision::backend_listener::BackendPeerIdentity>,
    /// Complete asset indices committed only after successful timely delivery.
    pub(super) verified_assets: usize,
    /// Worker candidate remains unqualified until its actual job result is consumed.
    candidate_assets: usize,
    /// Final metadata confirmation applies to the complete retained cohort.
    candidate_complete: bool,
}
impl Transaction {
    /// Test boundary custody participates in the same production lifetime checks.
    fn empty(&self) -> bool {
        #[cfg(test)]
        if self.fixture_slots != [0; 3] {
            return false;
        }
        self.http.empty()
    }
}
pub(super) struct FundedTransaction {
    pub(super) value: Box<Transaction>,
    pub(super) _claim: super::memory::BodyClaim,
}
pub(super) struct Shared {
    pub(super) transaction: Mutex<Option<FundedTransaction>>,
}
impl Drop for Shared {
    fn drop(&mut self) {
        let state = self.transaction.get_mut().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(value) = state.take() {
            if value.value.empty() {
                drop(value);
            } else {
                std::mem::forget(value);
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
    /// Actual timely partial delivery retains its next phase requirement; pending work clears it.
    continuation_headroom: Option<Duration>,
    client: Option<ExecutionClient>,
    execution: Option<sugarsubstitute_contracts::execution::ExecutionId>,
    /// The genuine original attempt scope remains the sole continuation destination.
    attempt_scope: Option<ScopeId>,
}
impl ReadinessProbeOwner {
    /// Construct real HTTP custody with a concrete external HTTP boundary fake.
    #[cfg(test)]
    pub(crate) fn from_fixture(
        input: ReadinessProbeInput,
        port: Arc<super::campaign_fixture::FixtureHttp>,
    ) -> Self {
        let mut owner = Self::new(input);
        owner.port = port;
        owner
    }
    /// Request cancellation while retaining the actual finite job and native custody.
    pub(crate) fn request_cancel(&self) {
        if let Some(job) = &self.pending {
            job.cancel();
        }
    }
    /// Prepare inert retained custody before admission; this performs no HTTP or process query.
    pub fn new(input: ReadinessProbeInput) -> Self {
        Self {
            input: Arc::new(input),
            shared: Arc::new(Shared { transaction: Mutex::new(None) }),
            port: Arc::new(super::native::SystemPort),
            pending: None,
            submitted: false,
            continuation_headroom: None,
            client: None,
            execution: None,
            attempt_scope: None,
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
        self.submit_step(client, scope, spec, false)
    }
    /// Resume only this identical retained owner after a checked partial success.
    /// Admission supplies a new finite phase; the original claim/cursor are reused.
    pub(crate) fn submit_continuation(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
    ) -> Result<JobId, AdmissionError> {
        validate(spec)?;
        if self.continuation_headroom.is_none()
            || self.pending.is_some()
            || self.execution != Some(scope.owner_id())
            || self.attempt_scope != Some(scope.id())
        {
            return Err(invalid("Readiness continuation unavailable or foreign"));
        }
        self.submit_step(client, scope, spec, true)
    }
    /// Admit one actual phase without replacing unresolved native outputs or their claim.
    fn submit_step(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
        continuation: bool,
    ) -> Result<JobId, AdmissionError> {
        let shared = self.shared.clone();
        let input = self.input.clone();
        let port = self.port.clone();
        let job = client.submit_blocking(scope, spec, move |context| {
            checkpoint(&context)?;
            let mut guard = lock(&shared.transaction);
            if !continuation {
                if guard.is_some() {
                    return Err(ReadinessTransportError::invalid("attempt_duplicate_custody"));
                }
                let claim = super::memory::BodyClaim::new(&context)
                    .map_err(|cause| ReadinessTransportError::new("attempt_memory", cause))?;
                let tcp =
                    sugarsubstitute_supervision::backend_listener::TcpOwnerWorkspace::new(&context)
                        .map_err(|cause| {
                            ReadinessTransportError::new("attempt_tcp_memory", cause)
                                .retain_body(claim.clone())
                        })?;
                *guard = Some(FundedTransaction {
                    value: Box::new(Transaction {
                        http: sugarsubstitute_networking::Transport::default(),
                        #[cfg(test)]
                        fixture_slots: [0; 3],
                        tcp,
                        deadline: context
                            .deadline()
                            .ok_or_else(|| ReadinessTransportError::invalid("attempt_deadline"))?,
                        peer: None,
                        verified_assets: 0,
                        candidate_assets: 0,
                        candidate_complete: false,
                    }),
                    _claim: claim,
                });
            }
            let state = guard
                .as_mut()
                .ok_or_else(|| ReadinessTransportError::invalid("attempt_custody"))?;
            let body = state._claim.clone();
            let state = &mut state.value;
            let result = (|| {
                if !state.empty() {
                    return Err(ReadinessTransportError::invalid("attempt_unsettled_predecessor"));
                }
                state.deadline = context
                    .deadline()
                    .ok_or_else(|| ReadinessTransportError::invalid("attempt_deadline"))?;
                state.candidate_assets = state.verified_assets;
                state.candidate_complete = false;
                port.begin(state, input.port(), &context)?;
                let (assets, complete) =
                    super::probe::inspect(port.as_ref(), state, &input, &context)?;
                port.close(state, &context)?;
                checkpoint(&context)?;
                state.candidate_assets = assets;
                state.candidate_complete = complete;
                Ok(())
            })();
            result.map_err(|cause: ReadinessTransportError| cause.retain_body(body))
        })?;
        self.pending = Some(job);
        self.submitted = true;
        self.continuation_headroom = None;
        self.client = Some(client.clone());
        self.execution = Some(scope.owner_id());
        self.attempt_scope = Some(scope.id());
        Ok(spec.id)
    }
    /// Passive readiness to admit another same-owner phase; grants no verified readiness.
    pub(crate) fn continuation_ready(&self) -> bool {
        self.continuation_headroom.is_some() && self.pending.is_none()
    }
    /// Ask whether this actual partial cohort can perform work inside the supplied unchanged phase.
    /// Metadata-only confirmation remains admissible below the asset retirement reserve.
    pub(crate) fn continuation_fits(&self, now: Moment, until: Moment) -> bool {
        self.pending.is_none()
            && self.continuation_headroom.is_some_and(|headroom| {
                until.checked_duration_since(now).is_ok_and(|remaining| remaining > headroom)
            })
    }
    /// Consume this owner's actual completion and qualify fresh delivery before creating readiness proof.
    /// # Errors
    /// Rejects cancellation, failure, late delivery, missing peer or retired installed sources.
    /// A timely partial cohort returns `None`; `ReadinessCampaign` drives its same-owner continuation.
    pub fn take_readiness(&mut self) -> Result<Option<VerifiedReadiness>, ReadinessTransportError> {
        let result = (|| {
            let Some(completion) = self.pending.as_mut().and_then(OwnedJob::try_take_completion)
            else {
                return Ok(None);
            };
            self.pending = None;
            self.continuation_headroom = None;
            tracing::info!(operation="readiness_attempt_terminal",job=?completion.spec.id,
            scope=?completion.scope_id,outcome=?completion.outcome.kind(),
            "Actual owned readiness attempt completion consumed");
            super::terminal::observe(completion.outcome)?;
            let client = self
                .client
                .as_ref()
                .ok_or_else(|| ReadinessTransportError::invalid("attempt_no_executor"))?;
            let now = client
                .now()
                .map_err(|e| ReadinessTransportError::new("attempt_delivery_clock", e))?;
            let mut guard = lock(&self.shared.transaction);
            let state = guard
                .as_mut()
                .ok_or_else(|| ReadinessTransportError::invalid("attempt_no_custody"))?;
            let state = &mut state.value;
            if !state.empty() {
                return Err(ReadinessTransportError::invalid("attempt_retirement_incomplete"));
            }
            if now >= state.deadline {
                return Err(ReadinessTransportError::elapsed(
                    "attempt_delivery_expired",
                    std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "Readiness probe delivery expired",
                    ),
                ));
            }
            self.input
                .source
                .validate_live()
                .map_err(|e| ReadinessTransportError::new("expected_source_retired", e))?;
            let peer =
                state.peer.ok_or_else(|| ReadinessTransportError::invalid("attempt_no_peer"))?;
            state.verified_assets = state.candidate_assets;
            if !state.candidate_complete {
                let total = self.input.source.manifest().assets().len();
                self.continuation_headroom = Some(if state.verified_assets < total {
                    super::probe::asset_headroom(total)
                } else {
                    Duration::ZERO
                });
                return Ok(None);
            }
            Ok(Some(VerifiedReadiness { input: self.input.clone(), peer }))
        })();
        result.map_err(|cause: ReadinessTransportError| {
            let guard = lock(&self.shared.transaction);
            match guard.as_ref() {
                Some(state) => cause.retain_body(state._claim.clone()),
                None => cause,
            }
        })
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
            let body = guard.as_ref().map(|state| state._claim.clone());
            let result = (|| {
                if let Some(state) = guard.as_mut() {
                    port.close(&mut state.value, &context)?;
                }
                if let Listener::Attached(lease, _) = &input.listener {
                    lease
                        .close(&context)
                        .map_err(|e| ReadinessTransportError::new("attached_identity_close", e))?;
                }
                *guard = None;
                Ok(())
            })();
            result.map_err(|cause: ReadinessTransportError| match body {
                Some(body) => cause.retain_body(body),
                None => cause,
            })
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
