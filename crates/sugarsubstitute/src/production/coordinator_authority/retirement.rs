// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep partial claims and constructor custody until actual finite closure is consumed.
use super::*;

impl CoordinatorAuthority {
    /// Fence ordinary authority without dropping actual initialization or constructor work.
    pub(crate) fn request_close(&mut self) {
        self.closing = true;
        self.serving = false;
        self.claims.close_admission();
        if let Some(registry) = self.registry_mut() {
            registry.close_launch_admission();
        }
        if self.claims.initialization_scope().is_none() {
            self.closed = true;
        }
    }
    /// Admit the existing checked native close under one immutable cleanup cutoff.
    /// Pending work is consumed first. A failed constructor/uncertain native close stays owned.
    pub(crate) fn submit_close(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
        until: Moment,
    ) -> Result<(), AuthorityIssue> {
        self.request_close();
        self.close_until = Some(self.close_until.map_or(until, |old| old.min(until)));
        if self.closed || self.close_job.is_some() {
            return Ok(());
        }
        match self.close_attempt {
            CloseAttempt::AdmissionRejected | CloseAttempt::Admitted => return Ok(()),
            CloseAttempt::WaitingCapacity => return Err(AuthorityIssue::Pending),
            CloseAttempt::Unissued => {}
        }
        if self.initialization.is_some() || self.registry_job.is_some() {
            return Err(AuthorityIssue::Pending);
        }
        let until = self.close_until.ok_or(AuthorityIssue::Pending)?;
        if self
            .registry_completion
            .as_ref()
            .is_some_and(|c| c.outcome.kind() != TerminalKind::Success)
        {
            if self.failed_registry.is_some() {
                self.close_attempt = CloseAttempt::AdmissionRejected;
                return Err(AuthorityIssue::Stopped);
            }
            // Keep the original failed constructor and its guard while one cleanup
            // constructor uses the same actual claims key and the original close cutoff.
            self.failed_registry = self.registry_completion.take();
        }
        if self.key.is_some() && self.registry_completion.is_none() {
            if let Err(issue) = self.construct_registry(client, scope, jobs, until) {
                self.close_attempt = CloseAttempt::AdmissionRejected;
                return Err(issue);
            }
            return Err(AuthorityIssue::Pending);
        }
        let spec = match self.spec(client, scope, jobs, CLAIM_OPERATION_BYTES, until) {
            Ok(spec) => spec,
            Err(issue) => {
                self.close_attempt = CloseAttempt::AdmissionRejected;
                return Err(issue);
            }
        };
        let result = match self.registry_completion.as_mut().map(|c| &mut c.outcome) {
            Some(JobOutcome::Success(registry)) => {
                if !self.registry_bound {
                    self.claims.bind_process_registry(registry).map_err(|cause| {
                        self.close_attempt = CloseAttempt::AdmissionRejected;
                        if self.cleanup_failure.is_none() {
                            self.cleanup_failure = Some(Failure::ClaimsAdmission(cause));
                        }
                        AuthorityIssue::Adapter
                    })?;
                    self.registry_bound = true;
                }
                self.claims.submit_close(client, scope, spec, registry)
            }
            Some(_) => return Err(AuthorityIssue::Stopped),
            None => self.claims.submit_close_unbound(client, scope, spec),
        };
        self.observe_close_admission(scope.id(), result)
    }
    /// Adopt the existing claims owner's checked admission result without
    /// equating rejected/pending metadata with an admitted native close job.
    pub(super) fn observe_close_admission(
        &mut self,
        scope: ScopeId,
        result: Result<JobId, ClaimAdmissionError>,
    ) -> Result<(), AuthorityIssue> {
        match result {
            Ok(id) => {
                self.close_job = Some((scope, id));
                self.close_attempt = CloseAttempt::Admitted;
            }
            Err(cause) => {
                if matches!(cause, ClaimAdmissionError::Busy) {
                    return Err(AuthorityIssue::Pending);
                }
                self.close_attempt = if matches!(
                    cause,
                    ClaimAdmissionError::Saturated
                        | ClaimAdmissionError::Execution(
                            sugarsubstitute_contracts::execution::AdmissionError::QueueSaturated { .. }
                                | sugarsubstitute_contracts::execution::AdmissionError::CompletionSaturated
                                | sugarsubstitute_contracts::execution::AdmissionError::MemorySaturated { .. }
                        )
                ) {
                    CloseAttempt::WaitingCapacity
                } else {
                    CloseAttempt::AdmissionRejected
                };
                self.fail(Failure::ClaimsAdmission(cause));
                return Err(AuthorityIssue::Adapter);
            }
        }
        Ok(())
    }
    /// Actual native closure, constructor observation and empty family custody all hold.
    pub(crate) fn settled(&self) -> bool {
        self.closing
            && self.closed
            && !self.pending()
            && self.registry().is_none_or(WindowsProcessRegistry::is_empty)
    }
}
