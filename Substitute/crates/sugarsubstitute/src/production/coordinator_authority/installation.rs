// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Keep installation admission and its original completion with the elected coordinator.
use super::*;
use std::sync::Arc;
use sugarsubstitute_contracts::execution::ExecutionPort;
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
    shell: shell::Shell,
    readiness_observed: bool,
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
            shell: shell::Shell::new(),
            readiness_observed: false,
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
    pub(super) fn consume(&mut self, batch: &CompletionBatch) -> Result<(), AuthorityIssue> {
        self.health.consume(batch)?;
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
    }

    fn application(&self) -> Option<&Arc<RunningApplication>> {
        match &self.completion.as_ref()?.outcome {
            JobOutcome::Success(value) => value.as_ref(),
            _ => None,
        }
    }

    /// Preserve the original ownership completion while the health writer borrows shared custody.
    pub(super) fn accept_ready(
        &mut self,
        client: &ExecutionClient,
        scope: &Scope,
        spec: JobSpec,
    ) -> Result<(), sugarsubstitute_contracts::execution::AdmissionError> {
        if self.needs_health()
            && let Some(application) = self.application().cloned()
        {
            self.health.start(client, scope, spec, application, publication::Decision::Accept)?;
        }
        Ok(())
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
