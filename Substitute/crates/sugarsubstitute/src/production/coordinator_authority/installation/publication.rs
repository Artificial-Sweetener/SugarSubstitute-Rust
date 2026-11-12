// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Own trial publication jobs and their observed terminals through coordinator retirement.
use super::*;
use std::io;
use sugarsubstitute_contracts::installation::TrialVerdict;

/// The caller supplies authenticated readiness or the verdict it classified for settlement.
#[derive(Clone, Copy, Debug)]
pub(super) enum Decision {
    /// Accept only after authenticated startup readiness.
    Accept,
    /// Preserve committed acceptance or settle the unresolved attempt with this verdict.
    Finish(TrialVerdict),
}

/// One publication job and its original terminal remain owned until coordinator retirement.
pub(super) struct Publication {
    job: Option<OwnedJob<(), io::Error>>,
    completion: Option<terminal::Terminal>,
    confirmed: bool,
}

impl Publication {
    /// Allocate inert state; no observation has authorized a publication.
    pub(super) fn new() -> Self {
        Self { job: None, completion: None, confirmed: false }
    }
    /// Repeated observations cannot issue a second publication, even after the first job fails.
    pub(super) fn unissued(&self) -> bool {
        self.job.is_none() && self.completion.is_none()
    }
    /// Shutdown must drain the real writer before releasing retained installation custody.
    pub(super) fn pending(&self) -> bool {
        self.job.is_some()
    }

    /// Follow-on work requires the original successful terminal with a matching notification.
    pub(super) fn succeeded(&self) -> bool {
        self.confirmed
            && self.completion.as_ref().is_some_and(|value| value.kind() == TerminalKind::Success)
    }

    /// The original admission completion retains another Arc through family teardown.
    pub(super) fn start(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
        application: Arc<RunningApplication>,
        decision: Decision,
    ) -> Result<(), sugarsubstitute_contracts::execution::AdmissionError> {
        self.job = Some(client.submit_blocking(scope, spec, move |context| {
            if let Some(trial) = application.trial() {
                let record = match decision {
                    Decision::Accept => trial.accept(&context),
                    Decision::Finish(verdict) => trial.finish_settled(verdict, &context),
                }
                .map_err(io::Error::other)?;
                tracing::info!(
                    operation = "installation_trial_publication",
                    decision = ?decision,
                    revision = record.revision(),
                    generation = trial.attempt().generation.id.as_str(),
                    "Published the observed installation trial decision"
                );
            }
            Ok(())
        })?);
        Ok(())
    }

    /// Match the exact scope/job/spec/outcome before retiring the publication job.
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
        let success = completion.outcome.kind() == TerminalKind::Success;
        self.confirmed = valid && success;
        self.completion = Some(terminal::Terminal::observe(completion));
        self.job = None;
        if !valid {
            Err(AuthorityIssue::Identity)
        } else if !success {
            Err(AuthorityIssue::Stopped)
        } else {
            Ok(())
        }
    }

    /// Keep the original storage failure available to the coordinator's failure presentation.
    pub(super) fn failure_source(&self) -> Option<&(dyn Error + 'static)> {
        self.completion.as_ref()?.failure_source()
    }
}

#[cfg(test)]
#[path = "publication/tests.rs"]
mod tests;
