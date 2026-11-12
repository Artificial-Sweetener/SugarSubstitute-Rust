// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Admit controller allocation on the existing finite blocking boundary before effects.
use super::*;

/// Log only closed categories and admission byte counts; retain original sensitive causes.
fn record_terminal(completion: &JobCompletion<PreparedController, PreparationFailure>) {
    let (cause, identity_stage, requested_bytes, available_bytes) = match &completion.outcome {
        JobOutcome::Failure(failure) => match &failure.cause {
            PreparationCause::Namespace(_) => ("namespace", None, None, None),
            PreparationCause::Memory(cause) => {
                ("memory", None, Some(cause.requested_bytes), Some(cause.available_bytes))
            }
            PreparationCause::Identity(stage) => ("identity", Some(stage), None, None),
            PreparationCause::Capacity => ("capacity", None, None, None),
        },
        _ => ("none", None, None, None),
    };
    tracing::error!(
        operation = "incident_constructor_terminal",
        scope = ?completion.scope_id,
        job = ?completion.spec.id,
        terminal = ?completion.outcome.kind(),
        preparation_cause = cause,
        identity_stage = ?identity_stage,
        requested_bytes,
        available_bytes,
        queue_time_ns = %completion.queue_time.as_nanos(),
        execution_time_ns = %completion.execution_time.as_nanos(),
        deadline_ns = completion.spec.deadline.map(Moment::as_nanos),
        "Incident controller preparation stopped"
    );
}

pub(super) fn allocate_policy(
    owner: CoordinatorKey,
    context: &JobContext,
) -> Result<FundedPolicy, PreparationCause> {
    let peak = IncidentCoordinator::required_storage_bytes()
        .checked_add(size_of::<PolicyState>() as u64)
        .ok_or(PreparationCause::Capacity)?;
    if peak > POLICY_BYTES {
        return Err(PreparationCause::Capacity);
    }
    let claim = context.memory().claim(POLICY_BYTES).map_err(PreparationCause::Memory)?;
    let recovery = super::super::diagnostic_helper::recovery::RecoveryService::new(context)
        .map_err(PreparationCause::Memory)?;
    let seed = super::super::diagnostic_helper::SeedService::new(context)
        .map_err(PreparationCause::Memory)?;
    let collection = super::super::diagnostic_helper::collection::CollectionService::new(context)
        .map_err(PreparationCause::Memory)?;
    let reporter_funding = Some(
        super::super::reporter_role::parent::ReporterFunding::new(context)
            .map_err(PreparationCause::Memory)?,
    );
    let reporting = super::super::diagnostic_helper::reporting::ReportingService::new(context)
        .map_err(PreparationCause::Memory)?;
    let exporting = super::super::diagnostic_helper::exporting::ExportService::new(context)
        .map_err(PreparationCause::Memory)?;
    let state = Box::new(PolicyState {
        coordinator: IncidentCoordinator::new(owner),
        effects: std::array::from_fn(|_| None),
    });
    Ok(FundedPolicy {
        state: Some(FundedState { state, _claim: claim }),
        recovery,
        seed,
        collection,
        reporting,
        exporting,
        reporter_funding,
    })
}
fn prepare(
    input: ChargedBlockingInput<PreparationInput>,
    context: JobContext,
) -> Result<PreparedController, PreparationFailure> {
    let result = (|| {
        let value = input.input();
        if value.namespace.namespace() != value.owner.namespace {
            return Err(PreparationCause::Identity(PreparationIdentity::Namespace));
        }
        if value.namespace.execution_owner() != context.owner_id() {
            return Err(PreparationCause::Identity(PreparationIdentity::ExecutionOwner));
        }
        if context.spec().reservation_bytes != CONTROLLER_BYTES {
            return Err(PreparationCause::Identity(PreparationIdentity::Reservation));
        }
        if context
            .deadline()
            .is_none_or(|until| until > value.namespace.original_preparation().until())
        {
            return Err(PreparationCause::Identity(PreparationIdentity::Deadline));
        }
        value.namespace.validate_live().map_err(PreparationCause::Namespace)?;
        allocate_policy(value.owner, &context)
    })();
    match result {
        Ok(policy) => Ok(PreparedController { policy, input }),
        Err(cause) => Err(PreparationFailure { cause, _input: input }),
    }
}
impl IncidentHost {
    /// Claim source and controller storage through the actual original finite admission.
    /// Rejection leaves the exact input occupied and consumes only a monotonic job ID.
    pub(in crate::production) fn submit_prepare(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
        until: Moment,
        input: &mut Option<PreparationInput>,
    ) -> Result<(), HostIssue> {
        if self.closing {
            return Err(HostIssue::Closed);
        }
        if self.constructor.is_some() || self.completion.is_some() {
            return Err(HostIssue::Started);
        }
        let value = input.as_ref().ok_or(HostIssue::Identity)?;
        if value.namespace.execution_owner() != scope.owner_id()
            || value.namespace.namespace() != value.owner.namespace
            || until > value.namespace.original_preparation().until()
        {
            return Err(HostIssue::Identity);
        }
        let id = match jobs.allocate() {
            Ok(id) => id,
            Err(cause) => {
                self.cause.get_or_insert(HostCause::Identity(cause));
                return Err(HostIssue::Identity);
            }
        };
        let spec = JobSpec {
            id,
            operation: Operation::NativeLifecycle,
            class: JobClass::InfrastructureLifecycle,
            deadline: Some(until),
            reservation_bytes: CONTROLLER_BYTES,
            replacement: None,
        };
        match client.submit_blocking_owned_box(scope, spec, input, prepare) {
            Ok(job) => {
                self.constructor = Some(job);
                Ok(())
            }
            Err(cause) => {
                self.cause.get_or_insert(HostCause::Admission(cause));
                Err(HostIssue::Admission)
            }
        }
    }
    /// Consume only actual matching terminal notices, then transfer the same completion once.
    pub(in crate::production) fn capture(
        &mut self,
        batch: &CompletionBatch,
    ) -> Result<bool, HostIssue> {
        let Some(job) = &mut self.constructor else {
            return Ok(false);
        };
        let mut matched = false;
        for notice in &batch.notifications {
            if (notice.scope_id, notice.job_id) != (job.scope_id(), job.spec().id) {
                continue;
            }
            if self.terminal.is_some_and(|old| old != notice.kind) {
                return Err(HostIssue::Identity);
            }
            self.terminal = Some(notice.kind);
            matched = true;
        }
        if self.terminal.is_some()
            && let Some(completion) = job.try_take_completion()
        {
            let valid = completion.scope_id == job.scope_id()
                && completion.spec == job.spec()
                && self.terminal == Some(completion.outcome.kind());
            self.completion = Some(completion);
            self.constructor = None;
            if !valid {
                return Err(HostIssue::Identity);
            }
            if !matches!(
                self.completion.as_ref().map(|value| &value.outcome),
                Some(JobOutcome::Success(_))
            ) {
                if let Some(completion) = &self.completion {
                    record_terminal(completion);
                }
                return Err(HostIssue::Preparation);
            }
        }
        Ok(matched)
    }
    /// Inspect actual successful construction; no phase marker substitutes for it.
    pub(in crate::production) fn prepared(&self) -> bool {
        !self.closing
            && matches!(
                self.completion.as_ref().map(|value| &value.outcome),
                Some(JobOutcome::Success(_))
            )
    }
}
