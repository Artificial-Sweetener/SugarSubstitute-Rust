// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Retire application versions nothing can start once this one is accepted, without letting
//! cleanup affect application health.
use super::*;
use std::{io, time::Duration};
use sugarsubstitute_supervision::setup_storage::{GenerationTurn, LaunchPointerGenerations};

/// Longest one cleanup turn may run; an interrupted turn resumes on the next launch.
const DEADLINE: Duration = Duration::from_secs(300);

/// One cleanup turn per coordinator lifetime, retaining admission failure or its terminal.
/// Cancellation and expiry drain normally; committed retirements resume on a later turn.
pub(super) struct Generations {
    issued: bool,
    job: Option<OwnedJob<(), io::Error>>,
    completion: Option<terminal::Terminal>,
}

impl Generations {
    /// Create inert state; nothing may be retired before this version is accepted.
    pub(super) fn new() -> Self {
        Self { issued: false, job: None, completion: None }
    }

    /// Whether the single turn has not been attempted yet.
    pub(super) fn unissued(&self) -> bool {
        !self.issued
    }

    /// Native work must actually return before coordinator custody can retire.
    pub(super) fn pending(&self) -> bool {
        self.job.is_some()
    }

    /// Start the turn once, borrowing the retained run custody of `application`.
    pub(super) fn start(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
        application: Arc<RunningApplication>,
    ) {
        if self.issued {
            return;
        }
        self.issued = true;
        match Self::admit(client, scope, jobs, application) {
            Ok(job) => self.job = Some(job),
            Err(cause) => tracing::warn!(
                operation = "installation_generation_retirement_admission_failed",
                cause = %crate::failure_details::Details(&cause),
                "Old versions stay until a later launch; cleanup could not start"
            ),
        }
    }

    fn admit(
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
        application: Arc<RunningApplication>,
    ) -> io::Result<OwnedJob<(), io::Error>> {
        let spec = JobSpec {
            id: jobs.allocate().map_err(io::Error::other)?,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureOrdinary,
            deadline: Some(
                client
                    .now()
                    .map_err(io::Error::other)?
                    .checked_add(DEADLINE)
                    .map_err(io::Error::other)?,
            ),
            reservation_bytes: setup_storage::GENERATION_RETIREMENT_RESERVATION_BYTES,
            replacement: None,
        };
        client
            .submit_blocking(scope, spec, move |context| {
                match setup_storage::retire_generations(
                    &application,
                    &LaunchPointerGenerations,
                    &context,
                ) {
                    Ok(GenerationTurn::Finished(_) | GenerationTurn::PointerUnreadable(_)) => {
                        Ok(())
                    }
                    Ok(GenerationTurn::PreparationBusy) => {
                        tracing::info!(
                            operation = "installation_generation_retirement_deferred",
                            "Old versions stay until a later launch; an update is being prepared"
                        );
                        Ok(())
                    }
                    Err(cause) => Err(io::Error::other(cause)),
                }
            })
            .map_err(io::Error::other)
    }

    /// Observe every outcome and keep its cause. Only mismatched completion identity is fatal.
    pub(super) fn consume(&mut self, batch: &CompletionBatch) -> Result<(), AuthorityIssue> {
        let Some(job) = &mut self.job else {
            return Ok(());
        };
        let Some(notice) = batch
            .notifications
            .iter()
            .find(|notice| (notice.scope_id, notice.job_id) == (job.scope_id(), job.spec().id))
        else {
            return Ok(());
        };
        let Some(completion) = job.try_take_completion() else {
            return Ok(());
        };
        let valid = completion.scope_id == job.scope_id()
            && completion.spec == job.spec()
            && completion.outcome.kind() == notice.kind;
        let kind = completion.outcome.kind();
        if let JobOutcome::Failure(cause) = &completion.outcome {
            tracing::warn!(
                operation = "installation_generation_retirement_failed",
                cause = %crate::failure_details::Details(cause),
                "Old versions stay until a later launch; cleanup failed"
            );
        } else if kind != TerminalKind::Success {
            tracing::info!(
                operation = "installation_generation_retirement_stopped",
                ?kind,
                "Old version cleanup stopped; it resumes on a later launch"
            );
        }
        self.completion = Some(terminal::Terminal::observe(completion));
        self.job = None;
        if valid { Ok(()) } else { Err(AuthorityIssue::Identity) }
    }
}

#[cfg(test)]
#[path = "generations/tests.rs"]
mod tests;
