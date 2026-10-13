// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Adopt exact typed completions before binding actual coordinator authority.
use super::*;

impl CoordinatorAuthority {
    /// Consume this same immutable shared batch; actual results retain independent guards.
    /// Returns true only after real claims and registry bind the actual elected owner.
    pub(crate) fn consume(
        &mut self,
        entry: &mut CoordinatorEntryOwner,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
        batch: &CompletionBatch,
    ) -> Result<bool, AuthorityIssue> {
        if batch.notifications.len() > 32 {
            return Err(AuthorityIssue::Identity);
        }
        if self.close_attempt == CloseAttempt::WaitingCapacity && !batch.notifications.is_empty() {
            self.close_attempt = CloseAttempt::Unissued;
        }
        let matched = match self.claims.consume(batch) {
            Ok(count) => count,
            Err(cause) => {
                self.fail(Failure::Claims(cause));
                0
            }
        };
        if matched != 0
            && self.initialization.is_some_and(|id| {
                batch.notifications.iter().any(|n| {
                    n.job_id == id && self.association.is_some_and(|(_, scope)| scope == n.scope_id)
                })
            })
        {
            self.initialization = None;
            self.initialization_finished = true;
        }
        if matched != 0
            && self.close_job.is_some_and(|identity| {
                batch.notifications.iter().any(|n| (n.scope_id, n.job_id) == identity)
            })
        {
            self.close_job = None;
        }
        // During startup these are only coordinator initialization/closure observations.
        // Later resource observers remain in the same claims owner's public queue.
        if !self.serving || self.close_job.is_some() {
            for _ in 0..32 {
                if self.observation_count == 32 {
                    break;
                }
                let Some(observation) = self.claims.take_observation() else { break };
                match observation {
                    ClaimObservation::CoordinatorClaimed(key) => {
                        if self.key.is_some_and(|old| old != key) {
                            return Err(AuthorityIssue::Identity);
                        }
                        self.key = Some(key);
                    }
                    ClaimObservation::Failed { target: ClaimTarget::Coordinator, cause } => {
                        self.fail(Failure::Claims(cause))
                    }
                    ClaimObservation::Stopped { target: ClaimTarget::Coordinator, kind } => {
                        self.stopped.get_or_insert(kind);
                        self.notice(AuthorityIssue::Stopped);
                        self.request_close();
                    }
                    ClaimObservation::NativeClaimsClosed => {
                        self.closed = true;
                        self.close_job = None;
                    }
                    other => self.retain_observation(other),
                }
            }
        }
        self.poll_registry(batch)?;
        if let Some(registry) = self.registry_mut()
            && let Err(cause) = registry.consume(batch)
        {
            self.fail(Failure::Registry(cause));
        }
        if self.closing {
            return self.consume_close_notice();
        }
        if self.initialization_finished && self.key.is_some() {
            self.construct_registry(client, scope, jobs, self.original.until())?;
        }
        if !self.serving && self.registry().is_some() {
            let registry =
                match &self.registry_completion.as_ref().ok_or(AuthorityIssue::Pending)?.outcome {
                    JobOutcome::Success(registry) => registry,
                    _ => return Err(AuthorityIssue::Stopped),
                };
            if let Err(cause) = self.claims.bind_process_registry(registry) {
                self.fail(Failure::ClaimsAdmission(cause));
                return Err(AuthorityIssue::Adapter);
            }
            self.registry_bound = true;
            if let Err(cause) = entry.bind_coordinator(
                &self.claims,
                registry,
                self.key.ok_or(AuthorityIssue::Identity)?,
            ) {
                self.fail(Failure::Entry(cause));
                return Err(AuthorityIssue::Adapter);
            }
            self.serving = true;
        }
        Ok(self.serving())
    }
    fn poll_registry(&mut self, batch: &CompletionBatch) -> Result<(), AuthorityIssue> {
        let Some(job) = &mut self.registry_job else { return Ok(()) };
        if !batch
            .notifications
            .iter()
            .any(|n| (n.scope_id, n.job_id) == (job.scope_id(), job.spec().id))
        {
            return Ok(());
        }
        let Some(completion) = job.try_take_completion() else { return Ok(()) };
        let valid = completion.scope_id == job.scope_id() && completion.spec == job.spec();
        let kind = completion.outcome.kind();
        self.registry_completion = Some(completion);
        self.registry_job = None;
        if !valid {
            self.request_close();
            return Err(AuthorityIssue::Identity);
        }
        if kind != TerminalKind::Success {
            self.constructor_failed_first |= self.failure.is_none();
            self.stopped.get_or_insert(kind);
            self.notice(AuthorityIssue::Stopped);
            self.request_close();
            return Err(AuthorityIssue::Stopped);
        }
        if self.closing
            && let Some(registry) = self.registry_mut()
        {
            registry.close_launch_admission();
        }
        Ok(())
    }
}
