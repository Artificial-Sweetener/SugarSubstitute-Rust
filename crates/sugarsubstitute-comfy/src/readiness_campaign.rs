// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Coordinate actual HTTP attempts and explicit cleanup under one original startup budget.
//!
//! The host supplies fresh installed-source/listener inputs only when requested,
//! routes its central completion batch here, and schedules native waits using
//! `next_deadline`. This owner creates no runtime, clocks, process authority or
//! sleeps. Retry admission follows actual partial native closure; successful
//! read-only attached identity remains retained until explicit final cleanup.
mod consume;
mod error;
#[cfg(test)]
mod tests;
use crate::readiness_transport::{
    ReadinessProbeInput, ReadinessProbeOwner, ReadinessTransportError, VerifiedReadiness,
};
pub use error::{CampaignError, CampaignRejection};
use sugarsubstitute_application::startup::{
    ConnectionBudget, ConnectionBudgetKind, ProbeBudgetError,
};
use sugarsubstitute_contracts::{
    configuration::LoopbackHost,
    execution::{ExecutionId, JobId, JobSpec, Moment, OwnedJob, Scope, ScopeId},
    lifecycle::{BackendTarget, ShutdownBudget},
};
use sugarsubstitute_execution::ExecutionClient;

/// Report immediate progression without implying native completion from queued work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CampaignStep {
    /// Actual attempt, cleanup or original backoff remains outstanding.
    Waiting,
    /// Previous native outputs closed; a fresh authoritative input may be supplied.
    NeedsInput,
    /// Actual completion was consumed and explicit cleanup is now required.
    Progressed,
    /// Actual source/HTTP/peer verification delivered current readiness.
    Ready,
    /// Admission is terminally closed; retained cleanup may remain outstanding.
    Failed,
}
struct Attempt {
    job: JobId,
    scope: ScopeId,
    until: Moment,
}
/// Own one real probe and one observed cleanup job across an original connection campaign.
pub struct ReadinessCampaign {
    target: BackendTarget,
    budget: ConnectionBudget,
    owner: Option<ReadinessProbeOwner>,
    attempt: Option<Attempt>,
    cleanup: Option<OwnedJob<(), ReadinessTransportError>>,
    execution: Option<ExecutionId>,
    cleanup_until: Option<Moment>,
    next_input_at: Option<Moment>,
    retry: bool,
    retry_index: usize,
    terminal: bool,
    shutdown: Option<ShutdownBudget>,
    last_now: Moment,
    readiness: Option<VerifiedReadiness>,
    ready: bool,
    failure: Option<CampaignError>,
}
impl ReadinessCampaign {
    /// Bind one effective target to its original thirty- or one-hundred-eighty-second budget.
    /// # Errors
    /// Rejects budget/target ownership mismatch and attached localhost/IPv6 origins.
    pub fn new(target: BackendTarget, budget: ConnectionBudget) -> Result<Self, CampaignError> {
        let matches = match &target {
            BackendTarget::Attached(origin) => {
                budget.kind() == ConnectionBudgetKind::Attached
                    && origin.host() == LoopbackHost::Ipv4
            }
            BackendTarget::Managed(_) => budget.kind() == ConnectionBudgetKind::Managed,
        };
        if !matches {
            return Err(CampaignError::Rejected(CampaignRejection::WrongTarget));
        }
        Ok(Self {
            target,
            budget,
            owner: None,
            attempt: None,
            cleanup: None,
            execution: None,
            cleanup_until: None,
            next_input_at: Some(budget.started()),
            retry: false,
            retry_index: 0,
            terminal: false,
            shutdown: None,
            last_now: budget.started(),
            readiness: None,
            ready: false,
            failure: None,
        })
    }
    /// Ask whether original time and actual predecessor settlement permit fresh input.
    /// # Errors
    /// Rejects clock regression and original campaign expiry without mutating state.
    pub fn needs_input(&self, now: Moment) -> Result<bool, CampaignError> {
        self.validate_now(now)?;
        if self.terminal || self.ready || self.owner.is_some() {
            return Ok(false);
        }
        self.budget.probe_until(now).map_err(CampaignError::Budget)?;
        Ok(self.next_input_at.is_some_and(|at| now >= at))
    }
    /// Submit one actual probe with fresh retained source and listener input.
    /// # Errors
    /// Rejects invalid bindings/order/deadlines before effects. Clock/admission
    /// failures close campaign admission. All original causes are returned intact.
    pub fn submit_attempt(
        &mut self,
        execution: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
        input: ReadinessProbeInput,
    ) -> Result<JobId, CampaignError> {
        let now = execution.now().map_err(|cause| {
            self.close_admission();
            CampaignError::Clock(cause)
        })?;
        self.observe_now(now)?;
        if !self.needs_input(now)? {
            return Err(CampaignError::Rejected(CampaignRejection::WrongPhase));
        }
        if !input.matches_target(&self.target) {
            return Err(CampaignError::Rejected(CampaignRejection::WrongTarget));
        }
        self.submit_owner(execution, scope, spec, ReadinessProbeOwner::new(input), now)
    }
    /// Admit actual retained custody after validating the original probe interval.
    fn submit_owner(
        &mut self,
        execution: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
        mut owner: ReadinessProbeOwner,
        now: Moment,
    ) -> Result<JobId, CampaignError> {
        self.observe_now(now)?;
        if !self.needs_input(now)? {
            return Err(CampaignError::Rejected(CampaignRejection::WrongPhase));
        }
        self.validate_scope(scope).inspect_err(|_| {
            self.terminal = true;
        })?;
        let until = self.budget.probe_until(now).map_err(CampaignError::Budget)?;
        if spec.deadline.is_none_or(|at| at <= now || at > until) {
            return Err(CampaignError::Rejected(CampaignRejection::InvalidSpecification));
        }
        let id = owner.submit_attempt(execution, scope, spec).map_err(|cause| {
            self.terminal = true;
            CampaignError::Admission(cause)
        })?;
        self.execution = Some(scope.owner_id());
        self.owner = Some(owner);
        self.attempt =
            Some(Attempt { job: id, scope: scope.id(), until: spec.deadline.unwrap_or(until) });
        self.next_input_at = None;
        self.last_now = now;
        Ok(id)
    }
    /// Submit explicit actual close on the original cleanup boundary.
    ///
    /// First retry cleanup freezes its supplied finite deadline within the
    /// original campaign. Cancellation/final cleanup uses the first shutdown's
    /// original settle deadline. Rejected/failed closes retain custody and their
    /// first deadline; a later request cannot renew it.
    /// # Errors
    /// Rejects pending work, foreign/closed scopes and invalid original deadlines;
    /// preserves actual admission failures without discarding the probe owner.
    pub fn submit_cleanup(
        &mut self,
        execution: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
    ) -> Result<JobId, CampaignError> {
        let now = execution.now().map_err(|cause| {
            self.close_admission();
            CampaignError::Clock(cause)
        })?;
        self.observe_now(now)?;
        self.validate_scope(scope)?;
        if self.attempt.is_some() || self.cleanup.is_some() || self.owner.is_none() {
            return Err(CampaignError::Rejected(CampaignRejection::WrongPhase));
        }
        let ancestor = self.shutdown.map_or(self.budget.until(), |budget| budget.settle_until);
        let until = self.cleanup_until.unwrap_or(ancestor);
        if spec.deadline.is_none_or(|at| at <= now || at > until) {
            return Err(CampaignError::Rejected(CampaignRejection::InvalidSpecification));
        }
        self.cleanup_until = Some(spec.deadline.unwrap_or(until));
        let owner =
            self.owner.as_ref().ok_or(CampaignError::Rejected(CampaignRejection::WrongPhase))?;
        let job = owner.submit_close(execution, scope, spec).map_err(|cause| {
            self.terminal = true;
            self.retry = false;
            CampaignError::Admission(cause)
        })?;
        self.cleanup = Some(job);
        self.last_now = now;
        Ok(spec.id)
    }
    /// Close admission and request actual attempt cancellation using the first shutdown budget.
    /// This does not close native outputs or establish settlement.
    pub fn cancel(&mut self, shutdown: ShutdownBudget) {
        if self.shutdown.is_none() {
            self.shutdown = Some(shutdown);
        }
        self.close_admission();
    }
    /// Return the next absolute original deadline or backoff wake in the same clock epoch.
    /// Retained terminal custody and actual cleanup keep their first deadline visible
    /// after readiness; live readiness has no remaining startup wake.
    pub fn next_deadline(&self) -> Option<Moment> {
        if let Some(attempt) = &self.attempt {
            Some(attempt.until.min(self.budget.until()))
        } else if self.owner.is_some() && (self.terminal || !self.ready || self.cleanup.is_some()) {
            Some(self.cleanup_until.unwrap_or_else(|| {
                self.shutdown.map_or(self.budget.until(), |budget| budget.settle_until)
            }))
        } else if !self.terminal && !self.ready {
            self.next_input_at.map(|at| at.min(self.budget.until()))
        } else {
            None
        }
    }
    /// Transfer actual readiness once; the probe retains attached identity until explicit cleanup.
    /// The host must revalidate this proof immediately before publication.
    pub fn take_readiness(&mut self) -> Option<VerifiedReadiness> {
        self.readiness.take()
    }
    /// Borrow the latest owned terminal or transient cause for structured diagnostics.
    pub fn failure(&self) -> Option<&CampaignError> {
        self.failure.as_ref()
    }
    /// Transfer the owned diagnostic cause without changing admission or native settlement.
    pub fn take_failure(&mut self) -> Option<CampaignError> {
        self.failure.take()
    }
    /// Observe actual probe/job retirement, independently of cancellation or failure classification.
    pub fn settled(&self) -> bool {
        self.owner.is_none() && self.attempt.is_none() && self.cleanup.is_none()
    }
    /// Reject a supplied regressing clock before changing campaign state.
    fn validate_now(&self, now: Moment) -> Result<(), CampaignError> {
        if now < self.last_now {
            Err(CampaignError::Rejected(CampaignRejection::ClockRegression))
        } else {
            Ok(())
        }
    }
    /// Fence mutating clock observations without renewing deadlines or retiring custody.
    fn observe_now(&mut self, now: Moment) -> Result<(), CampaignError> {
        if let Err(cause) = self.validate_now(now) {
            self.close_admission();
            if self.failure.is_none() {
                self.failure = Some(CampaignError::Rejected(CampaignRejection::ClockRegression));
            }
            return Err(cause);
        }
        Ok(())
    }
    /// Request actual pending cancellation while keeping jobs and native outputs owned.
    fn close_admission(&mut self) {
        self.terminal = true;
        self.retry = false;
        self.next_input_at = None;
        self.readiness = None;
        if let Some(owner) = &self.owner {
            owner.request_cancel();
        }
    }
    /// Keep all attempts and cleanup within one live execution lifetime.
    fn validate_scope(&self, scope: &Scope) -> Result<(), CampaignError> {
        if self.execution.is_some_and(|owner| owner != scope.owner_id()) {
            return Err(CampaignError::Rejected(CampaignRejection::ForeignExecution));
        }
        if scope.snapshot().closed || scope.snapshot().cancellation_requested {
            return Err(CampaignError::Rejected(CampaignRejection::ClosedScope));
        }
        Ok(())
    }
}
