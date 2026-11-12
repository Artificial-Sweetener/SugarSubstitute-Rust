// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep installation admission and its original completion with the elected coordinator.
use super::*;
use std::sync::Arc;
use sugarsubstitute_contracts::{
    execution::{AdmissionError, ExecutionPort},
    installation::{CloseCause, FailedProcess, TrialFacts, TrialVerdict},
};
use sugarsubstitute_supervision::setup_storage::{self, RunningApplication};
#[path = "installation/health.rs"]
mod health;
#[path = "installation/publication.rs"]
mod publication;
#[path = "installation/release.rs"]
mod release;
#[path = "installation/settlement.rs"]
mod settlement;
#[path = "installation/shell.rs"]
mod shell;
#[path = "installation/terminal.rs"]
mod terminal;
#[cfg(test)]
#[path = "installation/test_completion.rs"]
mod test_completion;
#[cfg(test)]
#[path = "installation/trial_tests.rs"]
mod trial_tests;

type Completion = JobCompletion<Option<Arc<RunningApplication>>, std::io::Error>;

impl CoordinatorAuthority {
    /// Share only the original observed installation owner with authenticated helper enrollment.
    pub(in crate::production) fn running_installation(&self) -> Option<Arc<RunningApplication>> {
        self.installation.application().cloned()
    }

    /// Background preparation requires authenticated readiness and observed trial acceptance.
    /// A cloned run owner is borrowed only by the finite executable-verification worker.
    pub(in crate::production) fn update_source(
        &self,
    ) -> Option<(CoordinatorKey, Arc<RunningApplication>)> {
        if !self.serving() || !self.installation.readiness_observed {
            return None;
        }
        let application = self.installation.application()?;
        if application.trial().is_some() && !self.installation.health.succeeded() {
            return None;
        }
        Some((self.key()?, application.clone()))
    }

    /// Record that `process` failed, for the verdict of an unaccepted trial. Call it where the
    /// failure is first observed; the first ending fact decides the verdict.
    pub(in crate::production) fn observe_trial_failure(&mut self, process: FailedProcess) {
        self.installation.facts.observe_failure(process);
    }

    /// Record that an owner asked this app to close for `cause`, for the verdict of an
    /// unaccepted trial. A failure observed after it is a consequence of closing.
    pub(in crate::production) fn observe_trial_close(&mut self, cause: CloseCause) {
        self.installation.facts.observe_close(cause);
    }

    /// Verdict an unaccepted trial would settle with now.
    #[cfg(test)]
    pub(in crate::production) fn trial_verdict(&self) -> TrialVerdict {
        self.installation.verdict()
    }

    /// Authenticated helper Finish may publish only after our installation workers retire.
    /// Observe actual terminals first, including failed/discarded health publication results.
    pub(in crate::production) fn installation_writers_retired(&self) -> bool {
        !self.installation.pending()
    }
}

/// Original worker lifetime; run guards retire only after checked coordinator teardown.
pub(super) struct Ownership {
    inherited: Option<setup_storage::InheritedLaunch>,
    job: Option<OwnedJob<Option<Arc<RunningApplication>>, std::io::Error>>,
    completion: Option<Completion>,
    health: publication::Publication,
    settlement: publication::Publication,
    /// Settles an unaccepted trial without a verdict as soon as Windows ends the session, so a
    /// forced end that follows finds it settled. The ordinary settlement still runs at close.
    session_end: publication::Publication,
    shell: shell::Shell,
    readiness_observed: bool,
    facts: TrialFacts,
    /// The health publication could not be admitted because execution capacity was full; the
    /// next completion batch, which releases capacity, retries it.
    health_waiting: bool,
    release: release::Release,
}

impl Ownership {
    /// Keep the initial inherited pin until election admits a worker or forwarding retires.
    pub(super) fn new(inherited: Option<setup_storage::InheritedLaunch>) -> Self {
        Self {
            inherited,
            job: None,
            completion: None,
            health: publication::Publication::new(),
            settlement: publication::Publication::new(),
            session_end: publication::Publication::new(),
            shell: shell::Shell::new(),
            readiness_observed: false,
            facts: TrialFacts::default(),
            health_waiting: false,
            release: release::Release::new(),
        }
    }
    /// Submit before child admission; native validation and hashing never run on the UI thread.
    pub(super) fn start(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
        generation: PathBuf,
    ) -> Result<(), sugarsubstitute_contracts::execution::AdmissionError> {
        let inherited = self.inherited.take();
        self.job = Some(client.submit_blocking(scope, spec, move |context| {
            setup_storage::retain_running_generation(&generation, inherited, &context)
                .map(|application| application.map(Arc::new))
        })?);
        Ok(())
    }

    /// Consume the exact worker terminal once, preserving all success and failure custody.
    /// A health publication that fails leaves the app open: its trial ends without a verdict
    /// at settlement, and its original cause stays retained for the report.
    pub(super) fn consume(&mut self, batch: &CompletionBatch) -> Result<(), AuthorityIssue> {
        if let Err(issue) = self.health.consume(batch) {
            tracing::warn!(
                operation = "installation_trial_acceptance_unresolved",
                issue = ?issue,
                cause = ?self.health.failure_source().map(ToString::to_string),
                "Trial acceptance was not published; the app stays open and the trial ends without a verdict"
            );
        }
        if let Err(issue) = self.session_end.consume(batch) {
            tracing::warn!(
                operation = "installation_trial_session_end_unsettled",
                issue = ?issue,
                cause = ?self.session_end.failure_source().map(ToString::to_string),
                "The session-end settlement did not publish; the ordinary settlement still runs"
            );
        }
        self.settlement.consume(batch)?;
        self.shell.consume(batch)?;
        let Some(job) = &mut self.job else { return Ok(()) };
        let Some(notice) = batch
            .notifications
            .iter()
            .find(|notice| (notice.scope_id, notice.job_id) == (job.scope_id(), job.spec().id))
        else {
            return Ok(());
        };
        let Some(completion) = job.try_take_completion() else { return Ok(()) };
        let valid = completion.scope_id == job.scope_id()
            && completion.spec == job.spec()
            && completion.outcome.kind() == notice.kind;
        let succeeded = completion.outcome.kind() == TerminalKind::Success;
        self.completion = Some(completion);
        self.job = None;
        if !valid {
            Err(AuthorityIssue::Identity)
        } else if !succeeded {
            Err(AuthorityIssue::Stopped)
        } else {
            Ok(())
        }
    }

    /// Admission succeeds only after the actual worker's complete successful outcome.
    pub(super) fn ready(&self) -> bool {
        self.completion
            .as_ref()
            .is_some_and(|result| matches!(result.outcome, JobOutcome::Success(_)))
    }

    /// A pending worker must drain even if no claims or child ever started.
    pub(super) fn pending(&self) -> bool {
        self.job.is_some()
            || self.health.pending()
            || self.settlement.pending()
            || self.session_end.pending()
            || self.shell.pending()
    }

    /// Preserve the original native/storage cause rather than reporting only a stopped job.
    pub(super) fn failure_source(&self) -> Option<&(dyn Error + 'static)> {
        if let Some(cause) =
            self.health.failure_source().or_else(|| self.settlement.failure_source())
        {
            return Some(cause);
        }
        match &self.completion.as_ref()?.outcome {
            JobOutcome::Failure(cause) => Some(cause),
            _ => None,
        }
    }

    /// Only the original successful admission can provide a trial needing health publication.
    pub(super) fn needs_health(&self) -> bool {
        self.health.unissued() && self.application().is_some_and(|value| value.trial().is_some())
    }

    /// Record only authenticated main/browser/backend readiness supplied by the parent owner.
    pub(super) fn observe_readiness(&mut self) {
        self.readiness_observed = true;
        self.facts.observe_readiness();
    }

    /// Verdict for settling an unaccepted trial, from the facts observed so far.
    pub(super) fn verdict(&self) -> TrialVerdict {
        self.facts.verdict()
    }

    fn application(&self) -> Option<&Arc<RunningApplication>> {
        match &self.completion.as_ref()?.outcome {
            JobOutcome::Success(value) => value.as_ref(),
            _ => None,
        }
    }

    /// Whether observed readiness still waits for its health publication to be admitted.
    pub(super) fn health_waiting(&self) -> bool {
        self.health_waiting && self.readiness_observed && self.needs_health()
    }

    /// Preserve the original ownership completion while the health writer borrows shared custody.
    /// Full execution capacity defers the publication to the next completion batch; any other
    /// admission refusal leaves acceptance unpublished and the app open.
    pub(super) fn accept_ready(&mut self, client: &ExecutionClient, scope: &Scope, spec: JobSpec) {
        let Some(application) = self.application().cloned().filter(|_| self.needs_health()) else {
            return;
        };
        let decision = publication::Decision::Accept;
        self.health_waiting = false;
        if let Err(cause) = self.health.start(client, scope, spec, application, decision) {
            self.health_waiting = matches!(
                cause,
                AdmissionError::QueueSaturated { .. }
                    | AdmissionError::CompletionSaturated
                    | AdmissionError::MemorySaturated { .. }
            );
            tracing::warn!(
                operation = "installation_trial_acceptance_deferred",
                cause = %cause,
                retry = self.health_waiting,
                "Trial acceptance could not start; the app stays open"
            );
        }
    }

    /// Publish shell access only after observed acceptance, with its own bounded worker lifetime.
    pub(super) fn publish_shell(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        jobs: &JobSequence,
    ) {
        let accepted_launch = self.readiness_observed
            && self.application().is_some_and(|application| application.trial().is_none());
        if (self.health.succeeded() || accepted_launch)
            && self.shell.unissued()
            && let Some(application) = self.application().cloned()
        {
            self.shell.start(client, scope, jobs, application);
        }
    }
}
