// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Compose actual held cleanup, finite verification and exact central terminal observations.
use super::{
    SymbolReportRequest,
    cause::{SymbolReportCause, SymbolReportFailure},
    source::Source,
    worker,
};
use std::time::Duration;
use sugarsubstitute_contracts::execution::{
    AdmissionError, CompletionBatch, JobClass, JobId, JobOutcome, JobSpec, Moment, Operation,
    OwnedJob, Scope, TerminalKind,
};
use sugarsubstitute_execution::{CleanupDeadline, ExecutionClient, SymbolCleanupPermit};
const MAIN_BYTES: u64 = 8_159_232;
const CLEANUP_BYTES: u64 = 32_768;
/// Return authenticated inert matching facts only after checked native retirement.
/// Failed outcomes retain original partial custody even when native closure is uncertain.
pub enum SymbolReportResult {
    /// Actual resolved frame after current-summary validation and checked cleanup.
    Verified(SymbolReportObservation),
    /// Same original source/capsule retained on semantic, worker or cleanup failure.
    Failed(SymbolReportFailure),
}
/// Exact committed identity plus actual resolved frame; no path or handle escapes.
pub struct SymbolReportObservation {
    pub(super) identity: (
        sugarsubstitute_contracts::incident::IncidentKey,
        sugarsubstitute_contracts::incident::IncidentRevision,
        sugarsubstitute_contracts::incident::DumpId,
    ),
    pub(super) frame: crate::incident::SymbolFrameObservation,
    pub(super) _source: Source,
}
impl SymbolReportObservation {
    /// Observe the sealed original Main owner and effective cutoff after genuine terminal cleanup.
    /// These inert facts grant no active work, native effect or replacement admission authority.
    /// # Errors
    /// Preserves the actual original admission error if its retained context is unavailable.
    pub fn original_admission(
        &self,
    ) -> Result<
        (
            sugarsubstitute_contracts::execution::ExecutionId,
            sugarsubstitute_contracts::execution::Moment,
        ),
        sugarsubstitute_contracts::execution::AdmissionError,
    > {
        self.frame.original_admission()
    }
    /// Exact current Summary revision observed under retained writer exclusion.
    pub const fn identity(
        &self,
    ) -> (
        sugarsubstitute_contracts::incident::IncidentKey,
        sugarsubstitute_contracts::incident::IncidentRevision,
        sugarsubstitute_contracts::incident::DumpId,
    ) {
        self.identity
    }
    /// Borrow the actual resolved failing frame and selected release association.
    pub fn frame(&self) -> &sugarsubstitute_contracts::incident::FailingFrame {
        self.frame.frame()
    }
}
/// Retain the two actual admitted jobs and one original closed metadata source.
/// The host supplies real independent scopes and globally allocated job identities.
pub struct SymbolReportService {
    client: ExecutionClient,
    main_spec: JobSpec,
    main: Option<OwnedJob<(), ()>>,
    held: Option<SymbolCleanupPermit<(), ()>>,
    cleanup: Option<OwnedJob<(), ()>>,
    cleanup_deadline: Option<CleanupDeadline>,
    main_terminal: bool,
    cleanup_terminal: bool,
    submitted: bool,
    source: Source,
}
impl SymbolReportService {
    /// Reserve actual independent cleanup and its fixed source before any native effect.
    /// `original` is the unchanged main cutoff, no more than sixty seconds from now.
    /// Immediately submit with the same main scope before allocating further job identities.
    /// # Errors
    /// Preserves actual original admission/clock failures; the metadata constructor does not
    /// run on rejection and no selected file, crypto handle or native provider is touched.
    pub fn reserve(
        client: ExecutionClient,
        cleanup_scope: &Scope,
        cleanup_id: JobId,
        main_scope: &Scope,
        main_id: JobId,
        original: Moment,
    ) -> Result<Self, AdmissionError> {
        let now = client.now().map_err(AdmissionError::Unexpected)?;
        let maximum = now
            .checked_add(Duration::from_secs(60))
            .map_err(|_| AdmissionError::InvalidSymbolAdmission)?;
        if original <= now || original > maximum {
            return Err(AdmissionError::InvalidSymbolAdmission);
        }
        let main_spec = JobSpec {
            id: main_id,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureOrdinary,
            deadline: Some(original),
            reservation_bytes: MAIN_BYTES,
            replacement: None,
        };
        let cleanup_spec = JobSpec {
            id: cleanup_id,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureLifecycle,
            deadline: None,
            reservation_bytes: CLEANUP_BYTES,
            replacement: None,
        };
        let cleanup_client = client.clone();
        let (source, held) = client
            .reserve_symbol_cleanup_with_source(
                cleanup_scope,
                cleanup_spec,
                main_scope,
                main_spec,
                move |claim| {
                    let source = Source::new(claim);
                    let cleanup_source = source.clone();
                    (source, move |context| {
                        worker::cleanup(cleanup_source, cleanup_client, context)
                    })
                },
            )
            .map_err(|rejection| rejection.error)?;
        Ok(Self {
            client,
            main_spec,
            main: None,
            held: Some(held),
            cleanup: None,
            cleanup_deadline: None,
            main_terminal: false,
            cleanup_terminal: false,
            submitted: false,
            source,
        })
    }
    /// Move an original namespace request into the preadmitted metadata source.
    /// Rejection preserves the caller's original occupied slot without allocating.
    /// # Errors
    /// Rejects empty, already bound or terminal requests without invoking native work.
    pub fn bind_request(
        &mut self,
        request: &mut Option<SymbolReportRequest>,
    ) -> Result<(), AdmissionError> {
        if self.submitted
            || self.main_terminal
            || self.source.with(|state| state.request.is_some()) != Some(false)
        {
            return Err(AdmissionError::InvalidSymbolAdmission);
        }
        let request = request.take().ok_or(AdmissionError::InvalidSymbolAdmission)?;
        let identity = (request.key, request.revision, request.dump);
        self.source.update(|state| {
            state.request = Some(request);
            state.identity = Some(identity);
        });
        Ok(())
    }
    /// Submit the bound original protected request through the actual paired Main.
    /// Rejected job admission retains the request in this same service for retry or cancellation.
    /// # Errors
    /// Rejects unbound/repeated requests and genuine closed, foreign or expired scopes.
    pub fn submit(&mut self, main_scope: &Scope) -> Result<(), AdmissionError> {
        if self.submitted
            || self.main_terminal
            || self.source.with(|state| state.request.is_some()) != Some(true)
        {
            return Err(AdmissionError::InvalidSymbolAdmission);
        }
        let held = self.held.as_ref().ok_or(AdmissionError::InvalidSymbolAdmission)?;
        let source = self.source.clone();
        let client = self.client.clone();
        match self.client.submit_symbol_helper(main_scope, self.main_spec, held, move |context| {
            worker::run(source, client, context)
        }) {
            Ok(job) => {
                self.main = Some(job);
                self.submitted = true;
                Ok(())
            }
            Err(rejection) => Err(rejection.error),
        }
    }
    /// Request cancellation of the actual main; wait for its exact terminal notification.
    pub fn cancel(&self) {
        if let Some(main) = &self.main {
            main.cancel();
        }
    }
    /// Cancel an unsubmitted held factory while retaining the same actual observation job.
    /// # Errors
    /// Rejects an already submitted or previously cancelled transaction.
    pub fn cancel_unsubmitted(&mut self) -> Result<(), AdmissionError> {
        if self.submitted || self.main_terminal {
            return Err(AdmissionError::InvalidSymbolAdmission);
        }
        let held = self.held.take().ok_or(AdmissionError::InvalidSymbolAdmission)?;
        self.cleanup = Some(held.cancel_and_observe());
        self.main_terminal = true;
        self.source.fail(SymbolReportCause::Stopped(TerminalKind::Cancelled));
        Ok(())
    }
    /// Consume only this service's exact genuine central completion notifications.
    /// Foreign metadata never takes a typed result or advances cleanup.
    pub fn capture(&mut self, batch: &CompletionBatch) -> bool {
        let mut changed = false;
        for notice in &batch.notifications {
            if self.main.as_ref().is_some_and(|job| {
                job.scope_id() == notice.scope_id && job.spec().id == notice.job_id
            }) && let Some(completion) =
                self.main.as_mut().and_then(OwnedJob::try_take_completion)
            {
                self.main_terminal = true;
                self.observe(completion.outcome);
                self.main = None;
                changed = true;
            }
            if self.cleanup.as_ref().is_some_and(|job| {
                job.scope_id() == notice.scope_id && job.spec().id == notice.job_id
            }) && let Some(completion) =
                self.cleanup.as_mut().and_then(OwnedJob::try_take_completion)
            {
                self.cleanup_terminal = true;
                self.observe(completion.outcome);
                if !self.submitted {
                    self.source.update(|state| state.settled = state.before_effects);
                }
                self.cleanup = None;
                changed = true;
            }
        }
        changed
    }
    fn observe(&self, outcome: JobOutcome<(), ()>) {
        match outcome {
            JobOutcome::Success(()) | JobOutcome::Failure(()) => {}
            JobOutcome::Panicked(cause) => self.source.fail(SymbolReportCause::Panic(cause)),
            stopped => self.source.fail(SymbolReportCause::Stopped(stopped.kind())),
        }
    }
    /// Activate the identical held job after actual main termination, without renewing time.
    /// # Errors
    /// Rejection preserves the same held permit, source and first immutable cleanup cutoff.
    pub fn advance_cleanup(&mut self) -> Result<bool, AdmissionError> {
        if !self.main_terminal || self.cleanup.is_some() || self.cleanup_terminal {
            return Ok(false);
        }
        let held = self.held.as_ref().ok_or(AdmissionError::InvalidSymbolAdmission)?;
        let original = match self.cleanup_deadline {
            Some(original) => original,
            None => {
                let original =
                    self.client.freeze_symbol_cleanup_deadline(held, Duration::from_secs(15))?;
                self.cleanup_deadline = Some(original);
                original
            }
        };
        let held = self.held.take().ok_or(AdmissionError::InvalidSymbolAdmission)?;
        match held.begin(original) {
            Ok(job) => {
                self.cleanup = Some(job);
                Ok(true)
            }
            Err(rejection) => {
                self.held = Some(rejection.permit);
                Err(rejection.error)
            }
        }
    }
    /// Take one terminal observation. Success requires actual checked physical settlement.
    /// Failure retains the same native capsule/claim and original causes through final Drop.
    pub fn take_result(&mut self) -> Option<SymbolReportResult> {
        if !self.cleanup_terminal {
            return None;
        }
        let (mut causes, mut data, settled, identity) = self.source.with(|state| {
            if state.taken {
                return None;
            }
            state.taken = true;
            Some((
                std::mem::take(&mut state.causes),
                state.data.take(),
                state.settled,
                state.identity,
            ))
        })??;
        if causes.iter().all(Option::is_none)
            && settled
            && data.as_ref().is_some_and(|data| data.first.is_none() && data.retirement.is_none())
            && let Some(frame) = data.as_mut().and_then(|data| data.observation.take())
            && let Some(identity) = identity
        {
            drop(data);
            return Some(SymbolReportResult::Verified(SymbolReportObservation {
                identity,
                frame,
                _source: self.source.clone(),
            }));
        }
        if causes.iter().all(Option::is_none)
            && data.as_ref().is_none_or(|data| data.first.is_none() && data.retirement.is_none())
        {
            causes[0] = Some(SymbolReportCause::State);
        }
        Some(SymbolReportResult::Failed(SymbolReportFailure {
            causes,
            data,
            settled,
            _source: self.source.clone(),
        }))
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
