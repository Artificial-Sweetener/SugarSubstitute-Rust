// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Read SugarSubstitute's published release notes on the startup execution owner.
//!
//! Each fetch is one admitted blocking job with a finite deadline, on the scope process
//! composition assigns. The job reads the changelog through the host's update HTTP owner with
//! its idle timeout, retires that owner's native custody before it returns, and nothing retries
//! on its own; the page's Retry asks again. One fetch runs at a time. Closing the application
//! cancels a running fetch, and shutdown waits until its outcome is observed.

use crate::job_sequence::JobSequence;
use std::error::Error;
use sugarsubstitute_contracts::{
    execution::{
        ExecutionPort, JobClass, JobContext, JobOutcome, JobSpec, Operation, OwnedJob, Scope,
        ScopeSpec,
    },
    host_release_notes::{ReleaseNotesFailure, ReleaseNotesVersion},
};
use sugarsubstitute_execution::ExecutionClient;
use sugarsubstitute_supervision::{
    release_notes::{
        self, ChangelogSource, RELEASE_NOTES_DEADLINE, RELEASE_NOTES_RESERVATION_BYTES, Selection,
    },
    update_http::Http,
};

/// Where the release notes of a range are read, one fetch at a time.
pub(crate) trait NotesFetches {
    /// Start reading the notes after `since` through `running`.
    ///
    /// # Errors
    /// Refuses while a fetch runs, and preserves the execution owner's refusal.
    fn start(
        &mut self,
        running: &ReleaseNotesVersion,
        since: &ReleaseNotesVersion,
    ) -> Result<(), Box<dyn Error>>;

    /// Take the running fetch's outcome once it has ended.
    fn take(&mut self) -> Option<Result<Selection, ReleaseNotesFailure>>;
}

/// No fetch can run: the application is closing, or composition assigned no fetch scope.
pub(crate) struct NoFetches;

impl NotesFetches for NoFetches {
    fn start(
        &mut self,
        _running: &ReleaseNotesVersion,
        _since: &ReleaseNotesVersion,
    ) -> Result<(), Box<dyn Error>> {
        Err("The release notes cannot be read now".into())
    }

    fn take(&mut self) -> Option<Result<Selection, ReleaseNotesFailure>> {
        None
    }
}

/// The startup host's release notes fetch job and its scope, created on the first fetch.
pub(super) struct ReleaseNotesJobs {
    spec: ScopeSpec,
    scope: Option<Scope>,
    job: Option<OwnedJob<Selection, ReleaseNotesFailure>>,
}

impl ReleaseNotesJobs {
    /// Fetches on the `spec` scope, which process composition assigns.
    pub(super) fn new(spec: ScopeSpec) -> Self {
        Self { spec, scope: None, job: None }
    }

    /// Whether no fetch is outstanding.
    pub(super) fn settled(&self) -> bool {
        self.job.is_none()
    }

    /// Ask a running fetch to stop; its outcome is still observed before shutdown ends.
    pub(super) fn cancel(&self) {
        if let Some(job) = &self.job {
            job.cancel();
        }
    }

    /// Bind this owner to the execution `client` and job identities for one turn.
    pub(super) fn port<'a>(
        &'a mut self,
        client: ExecutionClient,
        jobs: &'a JobSequence,
    ) -> ExecutionFetches<'a> {
        ExecutionFetches { owner: self, client, jobs }
    }
}

/// [`NotesFetches`] on the startup execution owner for one turn.
pub(super) struct ExecutionFetches<'a> {
    owner: &'a mut ReleaseNotesJobs,
    client: ExecutionClient,
    jobs: &'a JobSequence,
}

impl NotesFetches for ExecutionFetches<'_> {
    fn start(
        &mut self,
        running: &ReleaseNotesVersion,
        since: &ReleaseNotesVersion,
    ) -> Result<(), Box<dyn Error>> {
        if self.owner.job.is_some() {
            return Err("A release notes fetch is already running".into());
        }
        let scope = match self.owner.scope.take() {
            Some(scope) => scope,
            None => self.client.create_scope(self.owner.spec, None)?,
        };
        let spec = JobSpec {
            id: self.jobs.allocate()?,
            operation: Operation::Infrastructure,
            class: JobClass::InfrastructureOrdinary,
            deadline: Some(self.client.now()?.checked_add(RELEASE_NOTES_DEADLINE)?),
            reservation_bytes: RELEASE_NOTES_RESERVATION_BYTES,
            replacement: None,
        };
        let (running, since) = (running.as_str().to_owned(), since.as_str().to_owned());
        let submitted = self
            .client
            .submit_blocking(&scope, spec, move |context| fetch(&running, &since, &context));
        self.owner.scope = Some(scope);
        self.owner.job = Some(submitted?);
        Ok(())
    }

    fn take(&mut self) -> Option<Result<Selection, ReleaseNotesFailure>> {
        let completion = self.owner.job.as_mut()?.try_take_completion()?;
        self.owner.job = None;
        Some(match completion.outcome {
            JobOutcome::Success(selection) => Ok(selection),
            JobOutcome::Failure(failure) => Err(failure),
            other => {
                tracing::warn!(operation = "release_notes_fetch", outcome = ?other.kind(),
                    "The release notes fetch did not finish");
                Err(ReleaseNotesFailure::Unavailable)
            }
        })
    }
}

/// The blocking body of one fetch: read, then retire the HTTP owner's native custody.
fn fetch(
    running: &str,
    since: &str,
    context: &JobContext,
) -> Result<Selection, ReleaseNotesFailure> {
    let source = ChangelogSource::substitute().map_err(|cause| {
        tracing::error!(
            operation = "release_notes_source",
            ?cause,
            "The release notes source is invalid"
        );
        ReleaseNotesFailure::Unavailable
    })?;
    let mut http = Http::new();
    let outcome = release_notes::fetch(&mut http, &source, running, since, context);
    if !http.settled()
        && let Err(cause) = http.retire()
    {
        tracing::error!(operation = "release_notes_fetch", error = %cause,
            "The release notes request's HTTP custody could not be retired");
    }
    outcome
}
